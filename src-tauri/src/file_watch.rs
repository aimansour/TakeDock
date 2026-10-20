use crate::{
    adb::{self, ProcessHandle},
    discard::FileEvidence,
    helper::HelperClient,
    session::DeviceLease,
};
use std::{
    collections::{BTreeMap, HashSet},
    io::{BufRead, BufReader, Read},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, SyncSender},
    },
    thread,
    time::Duration,
};
use takedock_files::{
    Entry, validate_name,
    watch::{self, WatchBody, WatchFrame},
};

#[derive(Debug, Clone)]
pub enum WatchUpdate {
    Frame {
        frame: WatchFrame,
        evidence: Option<FileEvidence>,
    },
    Inspection {
        operation_id: String,
        entry: Result<Entry, String>,
    },
    Lost(String),
}
type Request = (String, String);
struct Shared {
    stop: Arc<AtomicBool>,
    active: AtomicBool,
    activity: AtomicU64,
    process: Mutex<Option<ProcessHandle>>,
    loss: Mutex<Option<String>>,
    pending: Mutex<HashSet<Request>>,
    events: SyncSender<WatchUpdate>,
}
impl Shared {
    fn fail(&self, code: String) {
        let mut loss = self.loss.lock().unwrap();
        if !self.stop.load(Ordering::Acquire) {
            *loss = Some(code);
            self.stop.store(true, Ordering::Release);
        }
    }
    fn send(&self, update: WatchUpdate) {
        if !self.stop.load(Ordering::Acquire) && self.events.try_send(update).is_err() {
            self.fail("file_watch_backpressure".into());
        }
    }
}
pub struct FileWatch {
    shared: Arc<Shared>,
    requests: SyncSender<Request>,
    workers: Mutex<Vec<thread::JoinHandle<()>>>,
}
impl FileWatch {
    pub fn is_stopped(&self) -> bool {
        self.shared.stop.load(Ordering::Acquire)
    }
    pub fn start(
        lease: DeviceLease,
        resources: PathBuf,
        clock: Arc<dyn Fn() -> u64 + Send + Sync>,
        callback: Arc<dyn Fn(WatchUpdate) + Send + Sync>,
    ) -> Self {
        let (events, receiver) = mpsc::sync_channel(128);
        let (requests, inspections) = mpsc::sync_channel::<Request>(8);
        let shared = Arc::new(Shared {
            stop: Arc::new(AtomicBool::new(false)),
            active: AtomicBool::new(false),
            activity: AtomicU64::new(clock()),
            process: Mutex::new(None),
            loss: Mutex::new(None),
            pending: Mutex::new(HashSet::new()),
            events,
        });
        let local = DeviceLease {
            cancelled: shared.stop.clone(),
            ..lease.clone()
        };
        let delivery = shared.clone();
        let pump = thread::spawn(move || {
            loop {
                let loss = delivery.loss.lock().unwrap().clone();
                if let Some(code) = loss {
                    callback(WatchUpdate::Lost(code));
                    break;
                }
                if delivery.stop.load(Ordering::Acquire) {
                    break;
                }
                match receiver.recv_timeout(Duration::from_millis(25)) {
                    Ok(event) => callback(event),
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(_) => break,
                }
            }
        });
        let supervise = shared.clone();
        let supervision_clock = clock.clone();
        let supervisor = thread::spawn(move || {
            while !supervise.stop.load(Ordering::Acquire) {
                if lease.cancelled.load(Ordering::Acquire) {
                    supervise.stop.store(true, Ordering::Release);
                    break;
                }
                if supervise.active.load(Ordering::Acquire)
                    && supervision_clock()
                        .saturating_sub(supervise.activity.load(Ordering::Acquire))
                        > 2500
                {
                    supervise.fail("file_watch_stale".into());
                    break;
                }
                thread::sleep(Duration::from_millis(10));
            }
            if let Some(process) = supervise.process.lock().unwrap().as_ref() {
                adb::kill(process);
            }
        });
        let reading = shared.clone();
        let watch_lease = local.clone();
        let watch_resources = resources.clone();
        let reader = thread::spawn(move || {
            let result = read_watch(&reading, &watch_lease, watch_resources, &clock);
            if let Err(error) = result {
                reading.fail(error);
            }
        });
        let inspecting = shared.clone();
        let inspector = thread::spawn(move || {
            let mut helper = HelperClient::new(resources);
            while !inspecting.stop.load(Ordering::Acquire) {
                let request = match inspections.recv_timeout(Duration::from_millis(25)) {
                    Ok(request) => request,
                    Err(mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(_) => break,
                };
                let entry = helper
                    .prepare(&local)
                    .and_then(|_| helper.output(&local, &["inspect", &request.1], 5000))
                    .and_then(|bytes| {
                        if bytes.len() > watch::MAX_FRAME {
                            return Err("file_watch_frame_too_large".into());
                        }
                        let entry: Entry =
                            serde_json::from_slice(&bytes).map_err(|_| "file_watch_protocol")?;
                        if entry.name != request.1 {
                            return Err("source_changed".into());
                        }
                        Ok(entry)
                    });
                inspecting.pending.lock().unwrap().remove(&request);
                inspecting.send(WatchUpdate::Inspection {
                    operation_id: request.0,
                    entry,
                });
            }
        });
        Self {
            shared,
            requests,
            workers: Mutex::new(vec![supervisor, reader, inspector, pump]),
        }
    }
    pub fn inspect(&self, operation_id: String, name: String) -> Result<(), String> {
        validate_name(&name)?;
        if self.shared.stop.load(Ordering::Acquire) {
            return Err("file_watch_closed".into());
        }
        let request = (operation_id, name);
        let mut pending = self.shared.pending.lock().unwrap();
        if !pending.insert(request.clone()) {
            return Ok(());
        }
        if self.requests.try_send(request.clone()).is_err() {
            pending.remove(&request);
            return Err("file_queue_full".into());
        }
        Ok(())
    }
    pub fn shutdown(&self) {
        self.shared.stop.store(true, Ordering::Release);
        for worker in self.workers.lock().unwrap().drain(..) {
            let _ = worker.join();
        }
    }
}
impl Drop for FileWatch {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn read_watch(
    shared: &Shared,
    lease: &DeviceLease,
    resources: PathBuf,
    clock: &Arc<dyn Fn() -> u64 + Send + Sync>,
) -> Result<(), String> {
    let mut helper = HelperClient::new(resources);
    helper.prepare(lease)?;
    if shared.stop.load(Ordering::Acquire) {
        return Ok(());
    }
    let stream = uuid::Uuid::new_v4().to_string();
    let command = helper.command(&["watch", &stream])?;
    let mut child = lease
        .adb
        .spawn(Some(&lease.serial), &["exec-out", &command], false)?;
    let output = child.stdout.take().unwrap();
    let errors = child.stderr.take().unwrap();
    let handle = Arc::new(Mutex::new(child));
    {
        let mut registered = shared.process.lock().unwrap();
        if shared.stop.load(Ordering::Acquire) {
            adb::kill(&handle);
            return Ok(());
        }
        *registered = Some(handle.clone());
    }
    let stderr = thread::spawn(move || {
        let _ = std::io::copy(&mut errors.take(64 * 1024), &mut std::io::sink());
    });
    shared.activity.store(clock(), Ordering::Release);
    shared.active.store(true, Ordering::Release);
    let mut output = BufReader::new(output);
    let mut evidence: Option<FileEvidence> = None;
    let mut sequence = 0;
    let result = (|| {
        loop {
            let mut line = Vec::new();
            output
                .by_ref()
                .take((watch::MAX_FRAME + 1) as u64)
                .read_until(b'\n', &mut line)
                .map_err(|_| "file_watch_closed")?;
            if shared.stop.load(Ordering::Acquire) {
                return Ok(());
            }
            if line.last() != Some(&b'\n') {
                return Err("file_watch_truncated_frame".into());
            }
            let frame = watch::decode_frame(&line)?;
            if frame.stream != stream || frame.sequence != sequence + 1 {
                return Err("file_watch_sequence_gap".into());
            }
            sequence = frame.sequence;
            let now = clock();
            match &frame.body {
                WatchBody::BaselineBegin => {
                    if evidence.as_ref().is_some_and(|e| e.complete) {
                        return Err("file_watch_protocol".into());
                    }
                    evidence = Some(FileEvidence {
                        stream: stream.clone(),
                        sequence,
                        complete: false,
                        received_ms: now,
                        entries: Arc::new(BTreeMap::new()),
                    });
                }
                WatchBody::BaselineEntry { entry } => {
                    let current = evidence
                        .as_mut()
                        .filter(|e| !e.complete)
                        .ok_or("file_watch_protocol")?;
                    if current.entries.len() >= watch::MAX_ENTRIES
                        || Arc::make_mut(&mut current.entries)
                            .insert(entry.name.clone(), entry.identity.clone())
                            .is_some()
                    {
                        return Err("file_watch_protocol".into());
                    }
                }
                WatchBody::BaselineEnd => {
                    evidence
                        .as_mut()
                        .filter(|e| !e.complete)
                        .ok_or("file_watch_protocol")?
                        .complete = true;
                }
                WatchBody::Created { entry } => {
                    let current = evidence
                        .as_mut()
                        .filter(|e| e.complete)
                        .ok_or("file_watch_protocol")?;
                    if current.entries.len() >= watch::MAX_ENTRIES
                        || Arc::make_mut(&mut current.entries)
                            .insert(entry.name.clone(), entry.identity.clone())
                            .is_some()
                    {
                        return Err("file_watch_protocol".into());
                    }
                }
                WatchBody::Changed { entry } | WatchBody::Closed { entry } => {
                    let current = evidence
                        .as_mut()
                        .filter(|e| e.complete)
                        .ok_or("file_watch_protocol")?;
                    if !current.entries.contains_key(&entry.name) {
                        return Err("file_watch_protocol".into());
                    }
                    Arc::make_mut(&mut current.entries)
                        .insert(entry.name.clone(), entry.identity.clone());
                }
                WatchBody::Removed { name } => {
                    let current = evidence
                        .as_mut()
                        .filter(|e| e.complete)
                        .ok_or("file_watch_protocol")?;
                    if Arc::make_mut(&mut current.entries).remove(name).is_none() {
                        return Err("file_watch_protocol".into());
                    }
                }
                WatchBody::Heartbeat => {
                    if evidence.as_ref().is_none_or(|e| !e.complete) {
                        return Err("file_watch_protocol".into());
                    }
                }
                WatchBody::Lost { code } => return Err(code.clone()),
            }
            if let Some(current) = &mut evidence {
                current.sequence = sequence;
                current.received_ms = now;
            }
            shared.activity.store(now, Ordering::Release);
            shared.send(WatchUpdate::Frame {
                frame,
                evidence: evidence.clone(),
            });
        }
    })();
    adb::kill(&handle);
    let _ = stderr.join();
    result
}
