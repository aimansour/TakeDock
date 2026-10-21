use super::{FileResultSnapshot, FileResults, ResultKey};
use serde_json::Value;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};

pub struct FileResultStore {
    model: Mutex<FileResults>,
    clock: Arc<dyn Fn() -> u64 + Send + Sync>,
    generation: AtomicU64,
}
impl FileResultStore {
    pub fn new(clock: Arc<dyn Fn() -> u64 + Send + Sync>) -> Self {
        Self {
            model: Mutex::new(FileResults::default()),
            clock,
            generation: AtomicU64::new(0),
        }
    }
    pub fn receive(&self, event: &str, payload: &Value) -> Option<u64> {
        if !matches!(
            event,
            "session-state"
                | "file-job"
                | "discard-result"
                | "file-index-warning"
                | "file-metadata-warning"
        ) {
            return None;
        }
        let generation = payload["generation"].as_u64()?;
        if event == "session-state" && generation <= self.generation.load(Ordering::Acquire) {
            return None;
        }
        let mut model = self.model.lock().unwrap();
        if !model.receive(event, payload, (self.clock)()) {
            return None;
        }
        if event == "session-state" {
            self.generation.fetch_max(generation, Ordering::Release);
        }
        Some(model.revision())
    }
    pub fn snapshot(&self) -> FileResultSnapshot {
        let model = self.model.lock().unwrap();
        model.snapshot((self.clock)())
    }
    pub fn acknowledge(&self, key: &ResultKey, revision: u64) -> Option<u64> {
        let mut model = self.model.lock().unwrap();
        model.acknowledge(key, revision).then(|| model.revision())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        events::Events,
        file_results::tests::{job, warning},
    };
    use serde_json::json;
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc,
    };
    use std::time::Duration;

    fn store() -> (Arc<FileResultStore>, Arc<AtomicU64>) {
        let clock = Arc::new(AtomicU64::new(0));
        let clock_value = clock.clone();
        let store = Arc::new(FileResultStore::new(Arc::new(move || {
            clock_value.load(Ordering::Relaxed)
        })));
        store.receive("session-state", &json!({"generation":7}));
        (store, clock)
    }
    #[test]
    fn snapshots_share_one_clock_and_ack() {
        let (first, clock) = store();
        let second = first.clone();
        first.receive("file-job", &job("a", "completed"));
        clock.store(9_999, Ordering::Relaxed);
        assert_eq!(first.snapshot().notices.len(), 1);
        assert_eq!(second.snapshot().notices[0].expires_at_ms, Some(10_000));
        clock.store(10_000, Ordering::Relaxed);
        assert!(first.snapshot().notices.is_empty());
        assert!(second.snapshot().notices.is_empty());
        assert!(first.clone().snapshot().notices.is_empty());
        first.receive("file-job", &job("failure", "failed"));
        let failure = second.snapshot().notices[0].clone();
        first.receive("session-state", &json!({"generation":8}));
        assert!(second.acknowledge(&failure.key, failure.revision).is_some());
        assert!(first.snapshot().notices.is_empty());
        assert!(first.acknowledge(&failure.key, failure.revision).is_none());
    }
    #[test]
    fn late_warning_does_not_lose_ack_race() {
        let (store, clock) = store();
        store.receive("file-job", &job("a", "completed"));
        let old = store.snapshot().notices[0].clone();
        store.receive("file-index-warning", &warning("a", "media_index_pending"));
        assert_eq!(store.acknowledge(&old.key, old.revision), None);
        clock.store(99_999, Ordering::Relaxed);
        assert_eq!(store.snapshot().notices[0].revision, 2);
    }
    #[test]
    fn replay_has_no_audio() {
        let (store, _) = store();
        let mut feedback = Events::default();
        feedback.receive("session-state", &json!({"generation":7}));
        let event = job("a", "completed");
        assert_eq!(feedback.receive("file-job", &event), Some(true));
        store.receive("file-job", &event);
        for _ in 0..3 {
            let notice = store.snapshot().history[0].clone();
            store.acknowledge(&notice.key, notice.revision);
            assert_eq!(store.receive("file-job", &event), None);
            assert_eq!(feedback.receive("file-job", &event), None);
        }
    }
    #[test]
    fn unchanged_capture_state_bypasses_result_lock() {
        let (store, _) = store();
        let guard = store.model.lock().unwrap();
        let other = store.clone();
        let (tx, rx) = mpsc::channel();
        let thread = std::thread::spawn(move || {
            tx.send(other.receive("session-state", &json!({"generation":7,"revision":999})))
                .unwrap();
        });
        let received = rx.recv_timeout(Duration::from_secs(1));
        drop(guard);
        thread.join().unwrap();
        assert_eq!(received.unwrap(), None);
    }
}
