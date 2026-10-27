//! Bounded best-effort metadata notifications, independent of file mutations.
use crate::session::{DeviceLease, EventSink};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, SyncSender},
    },
    thread,
};

type Action = Box<dyn FnOnce() + Send>;
pub(crate) struct IndexService {
    sender: SyncSender<Option<Action>>,
    stopped: Arc<AtomicBool>,
    worker: Mutex<Option<thread::JoinHandle<()>>>,
    sink: EventSink,
}
fn warning(sink: &EventSink, generation: u64, id: &str) {
    sink(
        "file-index-warning",
        serde_json::json!({"generation":generation,"id":id,"code":"media_index_pending"}),
    );
}
impl IndexService {
    pub(crate) fn start(sink: EventSink) -> Self {
        let (sender, receiver) = mpsc::sync_channel::<Option<Action>>(32);
        let stopped = Arc::new(AtomicBool::new(false));
        let stop = stopped.clone();
        let worker = thread::spawn(move || {
            while let Ok(Some(action)) = receiver.recv() {
                if !stop.load(Ordering::Acquire) {
                    action();
                }
            }
        });
        Self {
            sender,
            stopped,
            worker: Mutex::new(Some(worker)),
            sink,
        }
    }
    fn submit(&self, generation: u64, id: &str, action: impl FnOnce() + Send + 'static) -> bool {
        if self.stopped.load(Ordering::Acquire)
            || self.sender.try_send(Some(Box::new(action))).is_err()
        {
            warning(&self.sink, generation, id);
            false
        } else {
            true
        }
    }
    pub(crate) fn enqueue(
        &self,
        lease: DeviceLease,
        id: String,
        paths: Vec<(&'static str, String)>,
    ) {
        let sink = self.sink.clone();
        let stopped = self.stopped.clone();
        let key = id.clone();
        self.submit(lease.generation, &key, move || {
            for (root, name) in paths {
                if stopped.load(Ordering::Acquire) || lease.cancelled.load(Ordering::Acquire) {
                    warning(&sink, lease.generation, &id);
                    break;
                }
                let encoded = name
                    .as_bytes()
                    .iter()
                    .map(|b| {
                        if b.is_ascii_alphanumeric() || b"-._~".contains(b) {
                            (*b as char).to_string()
                        } else {
                            format!("%{b:02X}")
                        }
                    })
                    .collect::<String>();
                let uri = format!("file://{root}/{encoded}");
                if lease
                    .output(
                        &[
                            "shell",
                            "am",
                            "broadcast",
                            "-a",
                            "android.intent.action.MEDIA_SCANNER_SCAN_FILE",
                            "-d",
                            &uri,
                        ],
                        5000,
                    )
                    .is_err()
                {
                    warning(&sink, lease.generation, &id);
                    break;
                }
            }
        });
    }
    pub(crate) fn shutdown(&self) {
        if self.stopped.swap(true, Ordering::AcqRel) {
            return;
        }
        let _ = self.sender.send(None);
        if let Some(worker) = self.worker.lock().unwrap().take() {
            let _ = worker.join();
        }
    }
}
impl Drop for IndexService {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, mpsc};
    #[test]
    fn held_indexing_has_a_bounded_nonblocking_queue_and_owned_shutdown() {
        let (events, received) = mpsc::channel();
        let sink = Arc::new(move |name: &str, value: serde_json::Value| {
            events.send((name.to_owned(), value)).unwrap();
        });
        let index = IndexService::start(sink);
        let (started, ready) = mpsc::channel();
        let (release, held) = mpsc::channel();
        assert!(index.submit(7, "first", move || {
            started.send(()).unwrap();
            held.recv().unwrap();
        }));
        ready
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        for _ in 0..32 {
            assert!(index.submit(7, "queued", || {}));
        }
        assert!(!index.submit(7, "overflow", || panic!("overflow must not run")));
        let (name, warning) = received
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        assert_eq!(name, "file-index-warning");
        assert_eq!(warning["id"], "overflow");
        assert_eq!(warning["code"], "media_index_pending");
        release.send(()).unwrap();
        index.shutdown();
        assert!(!index.submit(7, "closed", || panic!("closed indexer must not run")));
    }
}
