//! Development-only observation. Default builds do not open files or read clocks.
#[cfg(any(test, feature = "transfer-diagnostics"))]
use std::path::Path;

#[derive(Clone, Copy, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Stage {
    Accepted,
    Enqueued,
    Worker,
    Prepare,
    InitialStat,
    Stream,
    Sync,
    StoredSize,
    PostTransferStat,
    Verify,
    RemoteHash,
    LocalHash,
    Publish,
    MoveStat,
    Delete,
    Index,
    Terminal,
}
#[derive(Clone, Copy, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Edge {
    Mark,
    Begin,
    #[cfg(feature = "transfer-diagnostics")]
    End,
}
pub(crate) type Tick = u64;

#[derive(Clone)]
pub(crate) struct TraceKey {
    #[cfg(feature = "transfer-diagnostics")]
    job_id: String,
    #[cfg(feature = "transfer-diagnostics")]
    generation: u64,
    #[cfg(feature = "transfer-diagnostics")]
    file_index: Option<usize>,
}
impl TraceKey {
    pub(crate) fn new(job_id: &str, generation: u64, file_index: Option<usize>) -> Self {
        #[cfg(feature = "transfer-diagnostics")]
        return Self {
            job_id: job_id.into(),
            generation,
            file_index,
        };
        #[cfg(not(feature = "transfer-diagnostics"))]
        {
            let _ = (job_id, generation, file_index);
            Self {}
        }
    }
}

#[derive(Clone, Default)]
pub(crate) struct Trace {
    #[cfg(feature = "transfer-diagnostics")]
    state: Option<std::sync::Arc<enabled::State>>,
}
pub(crate) struct Span {
    #[cfg(feature = "transfer-diagnostics")]
    trace: Trace,
    #[cfg(feature = "transfer-diagnostics")]
    key: TraceKey,
    #[cfg(feature = "transfer-diagnostics")]
    stage: Stage,
}
impl Drop for Span {
    fn drop(&mut self) {
        #[cfg(feature = "transfer-diagnostics")]
        self.trace.mark(&self.key, self.stage, Edge::End);
    }
}
impl Trace {
    pub(crate) fn from_env() -> Self {
        #[cfg(feature = "transfer-diagnostics")]
        {
            static TRACE: std::sync::OnceLock<Trace> = std::sync::OnceLock::new();
            TRACE
                .get_or_init(|| {
                    std::env::var_os("TAKEDOCK_TRANSFER_TRACE")
                        .map(|path| Self::for_path(Path::new(&path)))
                        .unwrap_or_default()
                })
                .clone()
        }
        #[cfg(not(feature = "transfer-diagnostics"))]
        Self {}
    }
    #[cfg(any(test, feature = "transfer-diagnostics"))]
    fn for_path(path: &Path) -> Self {
        #[cfg(feature = "transfer-diagnostics")]
        {
            if !path.is_absolute() {
                return Self::default();
            }
            let Some((frequency, kind, clock)) = enabled::clock() else {
                return Self::default();
            };
            let Ok(file) = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
            else {
                return Self::default();
            };
            Self::with_writer(file, clock, frequency, kind).0
        }
        #[cfg(not(feature = "transfer-diagnostics"))]
        {
            let _ = path;
            Self {}
        }
    }
    pub(crate) fn stamp(&self) -> Tick {
        #[cfg(feature = "transfer-diagnostics")]
        if let Some(state) = &self.state {
            return (state.clock)();
        }
        0
    }
    pub(crate) fn record_at(&self, key: &TraceKey, stage: Stage, edge: Edge, tick: Tick) {
        #[cfg(feature = "transfer-diagnostics")]
        if let Some(state) = &self.state {
            state.send(enabled::Message::Edge(key.clone(), stage, edge, tick));
        }
        #[cfg(not(feature = "transfer-diagnostics"))]
        let _ = (key, stage, edge, tick);
    }
    pub(crate) fn mark(&self, key: &TraceKey, stage: Stage, edge: Edge) {
        self.record_at(key, stage, edge, self.stamp());
    }
    pub(crate) fn span(&self, key: &TraceKey, stage: Stage) -> Span {
        self.mark(key, stage, Edge::Begin);
        #[cfg(feature = "transfer-diagnostics")]
        return Span {
            trace: self.clone(),
            key: key.clone(),
            stage,
        };
        #[cfg(not(feature = "transfer-diagnostics"))]
        Span {}
    }
    pub(crate) fn measure<T>(&self, key: &TraceKey, stage: Stage, action: impl FnOnce() -> T) -> T {
        let _span = self.span(key, stage);
        action()
    }
    pub(crate) fn complete_job(&self, key: &TraceKey) {
        #[cfg(feature = "transfer-diagnostics")]
        if let Some(state) = &self.state {
            state.send(enabled::Message::Complete(key.clone()));
        }
        #[cfg(not(feature = "transfer-diagnostics"))]
        let _ = key;
    }
    #[cfg(feature = "transfer-diagnostics")]
    fn with_writer(
        writer: impl std::io::Write + Send + 'static,
        clock: std::sync::Arc<dyn Fn() -> u64 + Send + Sync>,
        frequency: u64,
        kind: &'static str,
    ) -> (Self, std::thread::JoinHandle<()>) {
        enabled::start(writer, clock, frequency, kind)
    }
    #[cfg(all(test, feature = "transfer-diagnostics"))]
    fn dropped(&self) -> u64 {
        self.state
            .as_ref()
            .unwrap()
            .dropped
            .load(std::sync::atomic::Ordering::Relaxed)
    }
}

#[cfg(feature = "transfer-diagnostics")]
mod enabled {
    use super::*;
    use std::{
        io::{self, Write},
        sync::{
            Arc,
            atomic::{AtomicU64, Ordering},
            mpsc::{self, SyncSender},
        },
    };
    pub(super) enum Message {
        Edge(TraceKey, Stage, Edge, Tick),
        Complete(TraceKey),
    }
    pub(super) struct State {
        sender: SyncSender<Message>,
        pub(super) clock: Arc<dyn Fn() -> u64 + Send + Sync>,
        pub(super) dropped: Arc<AtomicU64>,
    }
    impl State {
        pub(super) fn send(&self, message: Message) {
            if self.sender.try_send(message).is_err() {
                self.dropped.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
    type Clock = Arc<dyn Fn() -> u64 + Send + Sync>;
    pub(super) fn clock() -> Option<(u64, &'static str, Clock)> {
        #[cfg(windows)]
        {
            #[link(name = "kernel32")]
            unsafe extern "system" {
                fn QueryPerformanceFrequency(frequency: *mut i64) -> i32;
                fn QueryPerformanceCounter(counter: *mut i64) -> i32;
            }
            let mut frequency = 0;
            // SAFETY: both Windows functions write exactly one valid i64.
            if unsafe { QueryPerformanceFrequency(&mut frequency) } == 0 || frequency <= 0 {
                return None;
            }
            let clock: Clock = Arc::new(|| {
                let mut tick = 0;
                // SAFETY: the pointer references a live, correctly sized i64.
                if unsafe { QueryPerformanceCounter(&mut tick) } == 0 || tick < 0 {
                    0
                } else {
                    tick as u64
                }
            });
            Some((frequency as u64, "qpc", clock))
        }
        #[cfg(not(windows))]
        {
            let start = std::time::Instant::now();
            Some((
                1_000_000_000,
                "process_relative",
                Arc::new(move || u64::try_from(start.elapsed().as_nanos()).unwrap_or(u64::MAX)),
            ))
        }
    }
    fn row(writer: &mut impl Write, value: serde_json::Value) -> io::Result<()> {
        serde_json::to_writer(&mut *writer, &value)?;
        writer.write_all(b"\n")?;
        writer.flush()
    }
    pub(super) fn start(
        mut writer: impl Write + Send + 'static,
        clock: Clock,
        frequency: u64,
        kind: &'static str,
    ) -> (Trace, std::thread::JoinHandle<()>) {
        let (sender, receiver) = mpsc::sync_channel(256);
        let dropped = Arc::new(AtomicU64::new(0));
        let writer_dropped = dropped.clone();
        let handle = std::thread::spawn(move || {
            if row(&mut writer, serde_json::json!({"schema":1,"record":"header","clock":kind,"frequency":frequency.to_string(),"process":std::process::id()})).is_err() { return; }
            for message in receiver {
                let value = match message {
                    Message::Edge(key, stage, edge, tick) => {
                        serde_json::json!({"record":"edge","job_id":key.job_id,"generation":key.generation.to_string(),"file_index":key.file_index,"stage":stage,"edge":edge,"ticks":tick.to_string()})
                    }
                    Message::Complete(key) => {
                        serde_json::json!({"record":"job_complete","job_id":key.job_id,"generation":key.generation.to_string(),"dropped":writer_dropped.load(Ordering::Relaxed),"writer_status":"ok"})
                    }
                };
                if row(&mut writer, value).is_err() {
                    break;
                }
            }
        });
        (
            Trace {
                state: Some(Arc::new(State {
                    sender,
                    clock,
                    dropped,
                })),
            },
            handle,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(feature = "transfer-diagnostics"))]
    #[test]
    fn disabled_trace_does_not_open_path() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("absent.jsonl");
        let trace = Trace::for_path(&path);
        let key = TraceKey::new("job", 7, None);
        trace.mark(&key, Stage::Accepted, Edge::Mark);
        trace.complete_job(&key);
        assert!(!path.exists());
        assert_eq!(std::mem::size_of::<Trace>(), 0);
    }

    #[cfg(feature = "transfer-diagnostics")]
    mod enabled {
        use super::*;
        use std::{
            io::{self, Write},
            sync::{
                Arc, Condvar, Mutex,
                atomic::{AtomicU64, Ordering},
            },
        };

        #[derive(Clone, Default)]
        struct Capture(Arc<Mutex<Vec<u8>>>);
        impl Write for Capture {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        impl Capture {
            fn rows(&self) -> Vec<serde_json::Value> {
                String::from_utf8(self.0.lock().unwrap().clone())
                    .unwrap()
                    .lines()
                    .map(|line| serde_json::from_str(line).unwrap())
                    .collect()
            }
        }
        fn clock() -> Arc<dyn Fn() -> u64 + Send + Sync> {
            let tick = AtomicU64::new(100);
            Arc::new(move || tick.fetch_add(1, Ordering::Relaxed))
        }
        fn finished(trace: Trace, writer: std::thread::JoinHandle<()>) {
            drop(trace);
            writer.join().unwrap();
        }

        #[test]
        fn trace_pairs_spans_on_error() {
            let output = Capture::default();
            let (trace, writer) = Trace::with_writer(output.clone(), clock(), 1000, "test");
            let key = TraceKey::new("job", 7, Some(0));
            let operation = || -> Result<(), &str> {
                let _span = trace.span(&key, Stage::Sync);
                Err("sync failed")
            };
            assert!(operation().is_err());
            trace.complete_job(&key);
            finished(trace, writer);
            let rows = output.rows();
            let edges: Vec<_> = rows.iter().filter(|row| row["stage"] == "sync").collect();
            assert_eq!(edges.len(), 2);
            assert_eq!(edges[0]["edge"], "begin");
            assert_eq!(edges[1]["edge"], "end");
            assert_eq!(edges[1]["ticks"], "101");
            assert_eq!(rows.last().unwrap()["record"], "job_complete");
            assert_eq!(rows.last().unwrap()["dropped"], 0);
        }

        #[test]
        fn trace_correlates_out_of_order_workers() {
            let output = Capture::default();
            let (trace, writer) = Trace::with_writer(output.clone(), clock(), 1000, "test");
            let key = TraceKey::new("same-job", u64::MAX, Some(1));
            trace.record_at(&key, Stage::Worker, Edge::Mark, 20);
            trace.record_at(&key, Stage::Enqueued, Edge::Mark, 10);
            trace.complete_job(&key);
            finished(trace, writer);
            let mut rows: Vec<_> = output
                .rows()
                .into_iter()
                .filter(|r| r["record"] == "edge")
                .collect();
            rows.sort_by_key(|row| row["ticks"].as_str().unwrap().parse::<u64>().unwrap());
            assert_eq!(rows[0]["stage"], "enqueued");
            assert_eq!(rows[1]["stage"], "worker");
            assert!(
                rows.iter()
                    .all(|r| r["job_id"] == "same-job" && r["file_index"] == 1)
            );
            assert_eq!(rows[0]["generation"], u64::MAX.to_string());
        }

        struct HeldWriter {
            gate: Arc<(Mutex<bool>, Condvar)>,
            entered: std::sync::mpsc::Sender<()>,
        }
        impl Write for HeldWriter {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                let _ = self.entered.send(());
                let (lock, ready) = &*self.gate;
                let mut open = lock.lock().unwrap();
                while !*open {
                    open = ready.wait(open).unwrap();
                }
                Ok(bytes.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        #[test]
        fn trace_overflow_invalidates_without_waiting() {
            let gate = Arc::new((Mutex::new(false), Condvar::new()));
            let (send, receive) = std::sync::mpsc::channel();
            let (trace, writer) = Trace::with_writer(
                HeldWriter {
                    gate: gate.clone(),
                    entered: send,
                },
                clock(),
                1000,
                "test",
            );
            receive
                .recv_timeout(std::time::Duration::from_secs(2))
                .unwrap();
            let key = TraceKey::new("job", 1, None);
            for _ in 0..300 {
                trace.mark(&key, Stage::Accepted, Edge::Mark);
            }
            let dropped = trace.dropped();
            let (lock, ready) = &*gate;
            *lock.lock().unwrap() = true;
            ready.notify_all();
            finished(trace, writer);
            assert!(
                dropped >= 44,
                "a full 256-record queue must invalidate timing"
            );
        }

        struct FailedWriter;
        impl Write for FailedWriter {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::Error::other("disk unavailable"))
            }
            fn flush(&mut self) -> io::Result<()> {
                Err(io::Error::other("disk unavailable"))
            }
        }
        #[test]
        fn trace_write_failure_does_not_fail_job() {
            let (trace, writer) = Trace::with_writer(FailedWriter, clock(), 1000, "test");
            writer.join().unwrap();
            let key = TraceKey::new("job", 1, None);
            trace.mark(&key, Stage::Terminal, Edge::Mark);
            trace.complete_job(&key);
            assert!(trace.dropped() >= 2);
        }

        #[test]
        fn trace_never_contains_private_names() {
            let output = Capture::default();
            let (trace, writer) = Trace::with_writer(output.clone(), clock(), 1000, "test");
            let key = TraceKey::new("job-123", 7, Some(0));
            trace.mark(&key, Stage::LocalHash, Edge::Begin);
            trace.mark(&key, Stage::LocalHash, Edge::End);
            trace.complete_job(&key);
            finished(trace, writer);
            let rows = output.rows();
            assert_eq!(rows[0]["schema"], 1);
            assert_eq!(rows[0]["frequency"], "1000");
            for row in rows {
                let keys = row.as_object().unwrap().keys();
                assert!(
                    keys.into_iter()
                        .all(|k| !["name", "path", "serial", "address"].contains(&k.as_str()))
                );
            }
        }
    }
}
