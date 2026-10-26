use crate::{
    adb::{self, Adb, ProcessHandle},
    model::*,
    readiness::{ReadinessPolicy, ReadyToken},
    recording::RecordingMachine,
};
use crate::{
    discard::{DiscardEffect, DiscardReceipt, TakeToken},
    file_watch::FileWatch,
    files::FileService,
};
use sha2::{Digest, Sha256};
mod discard_runtime;
use std::{
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, SyncSender},
    },
    thread,
    time::{Duration, Instant},
};

pub type EventSink = Arc<dyn Fn(&str, serde_json::Value) + Send + Sync>;
static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);
#[derive(Clone)]
pub struct DeviceLease {
    pub adb: Adb,
    pub serial: String,
    pub generation: u64,
    pub cancelled: Arc<AtomicBool>,
}
impl DeviceLease {
    pub fn output(&self, args: &[&str], timeout: u64) -> Result<Vec<u8>, String> {
        if self.cancelled.load(Ordering::Acquire) {
            return Err("session_replaced".into());
        }
        self.adb
            .output(&self.serial, args, timeout, &self.cancelled)
    }
}
struct QueuedCommand {
    ticket: Option<Arc<std::sync::atomic::AtomicU8>>,
    sequence: u64,
    action: RecordingAction,
}
struct Active {
    lease: DeviceLease,
    token: String,
    sender: SyncSender<QueuedCommand>,
    processes: Vec<ProcessHandle>,
}
struct Shared {
    services: Option<DiscardServices>,
    watcher: Mutex<Option<(u64, Arc<FileWatch>)>>,
    effects: SyncSender<(u64, DiscardEffect)>,
    adb: Adb,
    apk: PathBuf,
    settings: Settings,
    sink: EventSink,
    model: Mutex<RecordingMachine>,
    active: Mutex<Option<Active>>,
    stopped: AtomicBool,
    generation: AtomicU64,
    processes: Mutex<Vec<ProcessHandle>>,
    started: Instant,
}
pub struct Engine {
    shared: Arc<Shared>,
    workers: Mutex<Vec<thread::JoinHandle<()>>>,
}
#[derive(Clone)]
struct DiscardServices {
    resources: PathBuf,
    files: Arc<FileService>,
}

impl Shared {
    fn current(&self, generation: u64) -> bool {
        !self.stopped.load(Ordering::Acquire)
            && self.generation.load(Ordering::Acquire) == generation
    }
    fn now(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }
    fn state(&self) {
        // Stamp under the model lock; serialization/emission remain outside it.
        // Receivers can discard an older snapshot delivered by another thread.
        let (state, results) = {
            let mut model = self.model.lock().unwrap();
            (model.snapshot_for_event(), model.discard.drain_results())
        };
        for result in results {
            (self.sink)("discard-result", serde_json::to_value(result).unwrap());
        }
        (self.sink)("session-state", serde_json::to_value(state).unwrap());
    }
    fn results(&self, results: Vec<VerificationResult>) {
        for result in results {
            (self.sink)("verification-result", serde_json::to_value(result).unwrap());
        }
    }
    fn error(&self, code: &str, detail: &str) {
        (self.sink)(
            "operation-error",
            serde_json::json!({"generation":self.generation.load(Ordering::Acquire),"code":code,"detail":detail}),
        );
    }
    fn observer_failure(&self, generation: u64, code: &str, detail: &str) {
        let state = {
            let mut model = self.model.lock().unwrap();
            if model.state.generation != generation || !self.current(generation) {
                return;
            }
            // A live read/protocol error invalidates the evidence behind a cue.
            // Keep capture prediction and the authorized file lease unchanged.
            model.invalidate_speech_readiness();
            model.cancel_discard("discard_observation_lost");
            model.snapshot_for_event()
        };
        (self.sink)("session-state", serde_json::to_value(state).unwrap());
        (self.sink)(
            "operation-error",
            serde_json::json!({"generation":generation,"code":code,"detail":detail}),
        );
    }
    fn delivery_failed(&self, generation: u64, sequence: u64) {
        let mut model = self.model.lock().unwrap();
        if model.state.generation != generation {
            return;
        }
        let results = model.delivery_failed(sequence);
        drop(model);
        self.results(results);
        self.state();
    }
    fn register(&self, child: std::process::Child) -> ProcessHandle {
        let handle = Arc::new(Mutex::new(child));
        let mut processes = self.processes.lock().unwrap();
        processes.retain(|process| process.lock().unwrap().try_wait().ok().flatten().is_none());
        processes.push(handle.clone());
        handle
    }
    fn disconnect(&self, condition: &str) {
        let generation = NEXT_GENERATION.fetch_add(1, Ordering::AcqRel);
        let active = self.active.lock().unwrap().take();
        let discarded = {
            let mut model = self.model.lock().unwrap();
            model.cancel_discard("discard_disconnected");
            let snapshot = model.discard.snapshot();
            if snapshot.phase == crate::discard::DiscardPhase::Deleting
                && let Some(operation) = snapshot.operation_id
            {
                model.discard.finish_delete(
                    &operation,
                    crate::discard::DeleteOutcome::Unknown("discard_outcome_unknown".into()),
                );
            }
            let results = model.discard.drain_results();
            let policy = model.readiness_policy;
            self.generation.store(generation, Ordering::Release);
            *model = RecordingMachine::new(String::new(), generation);
            model.set_readiness_policy(policy);
            model.state.connected = false;
            model.state.condition = condition.into();
            results
        };
        for result in discarded {
            (self.sink)("discard-result", serde_json::to_value(result).unwrap());
        }
        self.state();
        if let Some(active) = active {
            active.lease.cancelled.store(true, Ordering::Release);
            if !active.processes.is_empty() {
                let stop = AtomicBool::new(false);
                let _ = active.lease.adb.output(
                    &active.lease.serial,
                    &[
                        "shell",
                        "am",
                        "broadcast",
                        "-a",
                        "app.takedock.observer.STOP",
                        "-p",
                        "app.takedock.observer",
                        "--es",
                        "session",
                        &active.token,
                    ],
                    1500,
                    &stop,
                );
                if let Some(observer) = active.processes.last() {
                    let deadline = Instant::now() + Duration::from_millis(500);
                    while Instant::now() < deadline
                        && observer.lock().unwrap().try_wait().ok().flatten().is_none()
                    {
                        thread::sleep(Duration::from_millis(10));
                    }
                }
                for process in &active.processes {
                    adb::kill(process);
                }
                let _ = active.lease.adb.output(
                    &active.lease.serial,
                    &["shell", "am", "force-stop", "app.takedock.observer"],
                    1000,
                    &stop,
                );
            }
        }
    }
    fn connect(self: &Arc<Self>, serial: String) -> Result<(), String> {
        let generation = NEXT_GENERATION.fetch_add(1, Ordering::AcqRel);
        let token = uuid::Uuid::new_v4().to_string();
        let lease = DeviceLease {
            adb: self.adb.clone(),
            serial,
            generation,
            cancelled: Arc::new(AtomicBool::new(false)),
        };
        let (sender, receiver) = mpsc::sync_channel::<QueuedCommand>(64);
        *self.active.lock().unwrap() = Some(Active {
            lease: lease.clone(),
            token: token.clone(),
            sender,
            processes: Vec::new(),
        });
        {
            let mut model = self.model.lock().unwrap();
            let policy = model.readiness_policy;
            self.generation.store(generation, Ordering::Release);
            *model = RecordingMachine::new(token.clone(), generation);
            model.set_readiness_policy(policy);
        }
        self.state();
        let hash = Sha256::digest(std::fs::read(&self.apk).map_err(|error| error.to_string())?)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let installed = lease
            .output(&["shell", "pm", "path", "app.takedock.observer"], 3000)
            .unwrap_or_default();
        let installed_path = std::str::from_utf8(&installed)
            .ok()
            .and_then(|text| text.lines().find_map(|line| line.strip_prefix("package:")))
            .map(str::to_owned);
        let installed_hash = installed_path.as_ref().and_then(|path| {
            let command = format!("sha256sum {}", shell_quote(path));
            lease
                .output(&["shell", &command], 4000)
                .ok()
                .and_then(|bytes| String::from_utf8(bytes).ok())
                .and_then(|text| text.split_whitespace().next().map(str::to_owned))
        });
        if installed_hash.as_deref() != Some(hash.as_str()) {
            let path = self.apk.to_str().ok_or("observer_path_encoding")?;
            let output = lease.output(&["install", "-r", path], 30_000)?;
            if !String::from_utf8_lossy(&output).contains("Success") {
                return Err("observer_install_failed".into());
            }
        }
        if !self.current(generation) {
            return Err("session_replaced".into());
        }
        // Lifecycle registration is atomic with respect to disconnect/shutdown.
        let mut active_guard = self.active.lock().unwrap();
        if !self.current(generation) {
            return Err("session_replaced".into());
        }
        let mut shell = lease
            .adb
            .spawn(Some(&lease.serial), &["shell", "-T"], true)?;
        let mut input = shell.stdin.take().unwrap();
        let shell_output = shell.stdout.take().unwrap();
        let shell_error = shell.stderr.take().unwrap();
        let shell_handle = self.register(shell);
        let mut observer = lease.adb.spawn(
            Some(&lease.serial),
            &[
                "shell",
                "am",
                "instrument",
                "-w",
                "-r",
                "-e",
                "session",
                &token,
                "app.takedock.observer/.ObserverInstrumentation",
            ],
            false,
        )?;
        let observer_output = observer.stdout.take().unwrap();
        let observer_error = observer.stderr.take().unwrap();
        let observer_handle = self.register(observer);
        active_guard.as_mut().ok_or("session_replaced")?.processes =
            vec![shell_handle, observer_handle];
        drop(active_guard);
        let shared = self.clone();
        thread::spawn(move || {
            while let Ok(command) = receiver.recv() {
                if !shared.current(generation) {
                    break;
                }
                if let Some(ticket) = &command.ticket {
                    let allowed = shared
                        .model
                        .lock()
                        .unwrap()
                        .commit_discard_write(command.sequence, ticket);
                    if !allowed {
                        continue;
                    }
                }
                let line = adb::capture_command(command.sequence, command.action);
                if input
                    .write_all(line.as_bytes())
                    .and_then(|_| input.flush())
                    .is_err()
                {
                    if shared.current(generation) {
                        shared.delivery_failed(generation, command.sequence);
                    }
                    break;
                }
            }
        });
        let shared = self.clone();
        thread::spawn(move || {
            for line in BufReader::new(shell_output).lines() {
                let Ok(line) = line else {
                    break;
                };
                if !shared.current(generation) {
                    break;
                }
                if let Some(marker) = line.strip_prefix("TD_DONE_")
                    && let Some((sequence, status)) = marker.split_once('_')
                    && status != "0"
                    && let Ok(sequence) = sequence.parse()
                {
                    shared.delivery_failed(generation, sequence);
                }
            }
            if shared.current(generation) {
                let mut model = shared.model.lock().unwrap();
                if model.state.generation != generation {
                    return;
                }
                model.channel_failed();
                drop(model);
                shared.state();
                shared.error("command_shell_closed", "");
            }
        });
        let shared = self.clone();
        thread::spawn(move || {
            for line in BufReader::new(observer_output).lines() {
                let Ok(line) = line else {
                    break;
                };
                if !shared.current(generation) {
                    break;
                }
                match adb::parse_observer_line(&line) {
                    Ok(Some(value)) if value["session"] == token => {
                        if value["type"] == "state" {
                            match serde_json::from_value::<Observation>(value) {
                                Ok(observation) => {
                                    let results = shared
                                        .model
                                        .lock()
                                        .unwrap()
                                        .observe(observation, shared.now());
                                    shared.results(results);
                                    shared.state();
                                }
                                Err(_) => shared.observer_failure(
                                    generation,
                                    "observer_protocol_invalid",
                                    "",
                                ),
                            }
                        } else if value["type"] == "error" {
                            shared.observer_failure(
                                generation,
                                "observer_failed",
                                value["message"].as_str().unwrap_or(""),
                            );
                        }
                    }
                    Ok(_) => {}
                    Err(_) => shared.observer_failure(generation, "observer_protocol_invalid", ""),
                }
            }
            if shared.current(generation) {
                let mut model = shared.model.lock().unwrap();
                if model.state.generation != generation {
                    return;
                }
                model.state.observer_ready = false;
                model.state.predicted = RecordingState::Unknown;
                model.state.condition = "observer_closed".into();
                model.invalidate_speech_readiness();
                model.cancel_discard("discard_observation_lost");
                drop(model);
                shared.error("observer_closed", "");
                shared.state();
            }
        });
        for errors in [shell_error, observer_error] {
            thread::spawn(move || {
                for line in BufReader::new(errors).lines() {
                    if line.is_err() {
                        break;
                    }
                }
            });
        }
        Ok(())
    }
}

impl Engine {
    pub fn start(settings: Settings, apk: PathBuf, sink: EventSink) -> Result<Self, String> {
        Self::start_inner(settings, apk, sink, None)
    }
    pub fn start_with_files(
        settings: Settings,
        apk: PathBuf,
        resources: PathBuf,
        files: Arc<FileService>,
        sink: EventSink,
    ) -> Result<Self, String> {
        Self::start_inner(
            settings,
            apk,
            sink,
            Some(DiscardServices { resources, files }),
        )
    }
    fn start_inner(
        settings: Settings,
        apk: PathBuf,
        sink: EventSink,
        services: Option<DiscardServices>,
    ) -> Result<Self, String> {
        let adb = Adb::resolve(&settings.adb_path)?;
        if !apk.is_file() {
            return Err("observer_resource_missing".into());
        }
        let generation = NEXT_GENERATION.fetch_add(1, Ordering::AcqRel);
        let mut model = RecordingMachine::new(String::new(), generation);
        model.set_readiness_policy(ReadinessPolicy {
            sound: settings.ready_sound,
            delay_ms: settings.ready_delay_ms,
        });
        model.state.connected = false;
        model.state.condition = "searching".into();
        let (effects, effect_receiver) = mpsc::sync_channel(32);
        let shared = Arc::new(Shared {
            services,
            watcher: Mutex::new(None),
            effects,
            adb,
            apk,
            settings,
            sink,
            model: Mutex::new(model),
            active: Mutex::new(None),
            stopped: AtomicBool::new(false),
            generation: AtomicU64::new(generation),
            processes: Mutex::new(Vec::new()),
            started: Instant::now(),
        });
        let timer = shared.clone();
        let timer_worker = thread::spawn(move || {
            while !timer.stopped.load(Ordering::Acquire) {
                let (results, changed, cue) = {
                    let mut model = timer.model.lock().unwrap();
                    let now = timer.now();
                    let results = model.expire(now);
                    let (changed, cue) = model.poll_speech_readiness(now);
                    (results, changed, cue)
                };
                if !results.is_empty() {
                    timer.results(results);
                    timer.state();
                } else if changed {
                    timer.state();
                }
                if let Some(token) = cue {
                    (timer.sink)("speech-ready", serde_json::to_value(token).unwrap());
                }
                timer.poll_discard();
                thread::sleep(Duration::from_millis(25));
            }
        });
        let tracker = shared.clone();
        let tracking_worker = thread::spawn(move || {
            while !tracker.stopped.load(Ordering::Acquire) {
                let Ok(mut process) = tracker.adb.spawn(None, &["track-devices"], false) else {
                    tracker.error("adb_tracking_failed", "");
                    thread::sleep(Duration::from_secs(1));
                    continue;
                };
                let output = process.stdout.take().unwrap();
                let errors = process.stderr.take().unwrap();
                tracker.register(process);
                thread::spawn(move || {
                    for line in BufReader::new(errors).lines() {
                        if line.is_err() {
                            break;
                        }
                    }
                });
                let mut output = BufReader::new(output);
                while let Ok(devices) = adb::read_tracking_frame(&mut output) {
                    if tracker.stopped.load(Ordering::Acquire) {
                        break;
                    }
                    let authorized = devices
                        .iter()
                        .filter(|(_, status)| status == "device")
                        .collect::<Vec<_>>();
                    let selected = if authorized.len() == 1 {
                        Some(authorized[0].0.clone())
                    } else {
                        None
                    };
                    let current = tracker
                        .active
                        .lock()
                        .unwrap()
                        .as_ref()
                        .map(|active| active.lease.serial.clone());
                    if current == selected && selected.is_some() {
                        continue;
                    }
                    let condition = if authorized.len() > 1 {
                        "multiple_devices"
                    } else if devices.iter().any(|(_, status)| status == "unauthorized") {
                        "unauthorized_device"
                    } else {
                        "no_device"
                    };
                    tracker.disconnect(condition);
                    if let Some(serial) = selected
                        && let Err(error) = tracker.connect(serial)
                        && !tracker.stopped.load(Ordering::Acquire)
                    {
                        let mut model = tracker.model.lock().unwrap();
                        model.state.observer_ready = false;
                        model.state.predicted = RecordingState::Unknown;
                        model.state.condition = "observer_setup_failed".into();
                        model.invalidate_speech_readiness();
                        drop(model);
                        tracker.state();
                        tracker.error("observer_setup_failed", &error);
                    }
                }
                if !tracker.stopped.load(Ordering::Acquire) {
                    tracker.disconnect("adb_tracking_closed");
                    tracker.error("adb_tracking_closed", "");
                    thread::sleep(Duration::from_secs(1));
                }
            }
        });
        let mut workers = vec![timer_worker, tracking_worker];
        if shared.services.is_some() {
            let effects = shared.clone();
            workers.push(thread::spawn(move || {
                effects.run_discard_effects(effect_receiver)
            }));
            let watchers = shared.clone();
            workers.push(thread::spawn(move || watchers.run_watch_manager()));
        }
        Ok(Self {
            shared,
            workers: Mutex::new(workers),
        })
    }
    pub fn snapshot(&self) -> SessionState {
        self.shared.model.lock().unwrap().snapshot_for_event()
    }
    pub fn lease(&self) -> Result<DeviceLease, String> {
        self.shared
            .active
            .lock()
            .unwrap()
            .as_ref()
            .map(|active| active.lease.clone())
            .ok_or("no_device".into())
    }
    pub fn lease_for(&self, generation: u64) -> Result<DeviceLease, String> {
        let lease = self.lease()?;
        if lease.generation != generation || !self.shared.current(generation) {
            return Err("session_replaced".into());
        }
        Ok(lease)
    }
    pub fn dispatch(&self, action: RecordingAction) -> Result<CommandReceipt, String> {
        self.dispatch_for(action, self.snapshot().generation)
    }
    pub fn dispatch_for(
        &self,
        action: RecordingAction,
        generation: u64,
    ) -> Result<CommandReceipt, String> {
        self.shared.dispatch_capture(action, generation, None)
    }
    pub fn request_discard(
        &self,
        take: TakeToken,
        generation: u64,
    ) -> Result<DiscardReceipt, String> {
        self.shared.request_discard(take, generation)
    }
    pub fn set_readiness_policy(&self, policy: ReadinessPolicy) {
        self.shared
            .model
            .lock()
            .unwrap()
            .set_readiness_policy(policy);
    }
    pub fn retire_handed_off(&self, update: &crate::files::HandoffUpdate) {
        if !self.shared.current(update.generation) {
            return;
        }
        let Ok(expected) =
            serde_json::from_str::<takedock_files::Identity>(&update.source_identity)
        else {
            return;
        };
        let mut model = self.shared.model.lock().unwrap();
        if model.state.generation != update.generation {
            return;
        }
        let changed = model.discard.handed_off(&update.source_name, &expected);
        model.withdraw_cancelled_discard();
        drop(model);
        if changed {
            self.shared.state();
        }
    }
    pub fn invalidate_speech_readiness(&self) {
        let mut model = self.shared.model.lock().unwrap();
        model.invalidate_speech_readiness();
        let state = model.snapshot_for_event();
        drop(model);
        (self.shared.sink)("session-state", serde_json::to_value(state).unwrap());
    }
    pub fn claim_ready_cue(&self, token: &ReadyToken) -> bool {
        let mut model = self.shared.model.lock().unwrap();
        if model.state.generation != token.generation || !self.shared.current(token.generation) {
            return false;
        }
        model.claim_ready_cue(token)
    }
    pub fn shutdown(&self) {
        if self.shared.stopped.swap(true, Ordering::AcqRel) {
            return;
        }
        self.shared.disconnect("disconnected");
        for process in self.shared.processes.lock().unwrap().iter() {
            adb::kill(process);
        }
        for worker in self.workers.lock().unwrap().drain(..) {
            let _ = worker.join();
        }
    }
}
impl Drop for Engine {
    fn drop(&mut self) {
        self.shutdown();
    }
}
pub fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_failure_cannot_resurrect_readiness() {
        let generation = NEXT_GENERATION.fetch_add(1, Ordering::AcqRel);
        let mut model = RecordingMachine::new("queue-test".into(), generation);
        let observation = |seq, state| Observation {
            version: 1,
            session: "queue-test".into(),
            seq,
            event_time: seq * 100,
            foreground: true,
            video_mode: true,
            state,
        };
        model.observe(observation(1, RecordingState::Idle), 0);
        model.dispatch(RecordingAction::Start, 0, 10_000).unwrap();
        model.observe(observation(2, RecordingState::Recording), 100);
        let token = model.poll_speech_readiness(900).1.unwrap();
        let (sender, _receiver) = mpsc::sync_channel(1);
        sender
            .try_send(QueuedCommand {
                ticket: None,
                sequence: 0,
                action: RecordingAction::Start,
            })
            .unwrap();
        let events = Arc::new(Mutex::new(Vec::new()));
        let output = events.clone();
        let adb = Adb {
            path: PathBuf::from("unused-test-peer"),
        };
        let engine = Engine {
            shared: Arc::new(Shared {
                services: None,
                watcher: Mutex::new(None),
                effects: mpsc::sync_channel(1).0,
                adb: adb.clone(),
                apk: PathBuf::new(),
                settings: Settings::default(),
                sink: Arc::new(move |event, value| {
                    output.lock().unwrap().push((event.to_owned(), value));
                }),
                model: Mutex::new(model),
                active: Mutex::new(Some(Active {
                    lease: DeviceLease {
                        adb,
                        serial: "fixture-queue".into(),
                        generation,
                        cancelled: Arc::new(AtomicBool::new(false)),
                    },
                    token: "queue-test".into(),
                    sender,
                    processes: Vec::new(),
                })),
                stopped: AtomicBool::new(false),
                generation: AtomicU64::new(generation),
                processes: Mutex::new(Vec::new()),
                started: Instant::now(),
            }),
            workers: Mutex::new(Vec::new()),
        };
        assert_eq!(
            engine
                .dispatch_for(RecordingAction::Stop, generation)
                .unwrap_err(),
            "command_queue_unavailable"
        );
        let snapshot = engine.snapshot();
        assert_eq!(snapshot.predicted, RecordingState::Recording);
        assert_eq!(snapshot.command_sequence, 1);
        assert_eq!(snapshot.speech_readiness, SpeechReadiness::Inactive);
        assert!(!engine.claim_ready_cue(&token));
        assert!(
            events
                .lock()
                .unwrap()
                .iter()
                .any(|(event, value)| event == "session-state"
                    && value["speech_readiness"] == "inactive")
        );
    }
}
