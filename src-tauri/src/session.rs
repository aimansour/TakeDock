use crate::{
    adb::{self, Adb, ProcessHandle},
    model::*,
    recording::RecordingMachine,
};
use sha2::{Digest, Sha256};
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

impl Shared {
    fn current(&self, generation: u64) -> bool {
        !self.stopped.load(Ordering::Acquire)
            && self.generation.load(Ordering::Acquire) == generation
    }
    fn now(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }
    fn state(&self) {
        let state = self.model.lock().unwrap().state.clone();
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
        self.generation.store(generation, Ordering::Release);
        let active = self.active.lock().unwrap().take();
        {
            let mut model = self.model.lock().unwrap();
            *model = RecordingMachine::new(String::new(), generation);
            model.state.connected = false;
            model.state.condition = condition.into();
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
        self.generation.store(generation, Ordering::Release);
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
            *self.model.lock().unwrap() = RecordingMachine::new(token.clone(), generation);
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
                                    let results = shared.model.lock().unwrap().observe(observation);
                                    shared.results(results);
                                    shared.state();
                                }
                                Err(_) => shared.error("observer_protocol_invalid", ""),
                            }
                        } else if value["type"] == "error" {
                            shared
                                .error("observer_failed", value["message"].as_str().unwrap_or(""));
                        }
                    }
                    Ok(_) => {}
                    Err(_) => shared.error("observer_protocol_invalid", ""),
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
        let adb = Adb::resolve(&settings.adb_path)?;
        if !apk.is_file() {
            return Err("observer_resource_missing".into());
        }
        let generation = NEXT_GENERATION.fetch_add(1, Ordering::AcqRel);
        let mut model = RecordingMachine::new(String::new(), generation);
        model.state.connected = false;
        model.state.condition = "searching".into();
        let shared = Arc::new(Shared {
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
                let results = timer.model.lock().unwrap().expire(timer.now());
                if !results.is_empty() {
                    timer.results(results);
                    timer.state();
                }
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
        Ok(Self {
            shared,
            workers: Mutex::new(vec![timer_worker, tracking_worker]),
        })
    }
    pub fn snapshot(&self) -> SessionState {
        self.shared.model.lock().unwrap().state.clone()
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
        let active = self.shared.active.lock().unwrap();
        let active = active.as_ref().ok_or("no_device")?;
        if active.lease.generation != generation || !self.shared.current(generation) {
            return Err("session_replaced".into());
        }
        let mut model = self.shared.model.lock().unwrap();
        let previous = model.clone();
        let receipt = model.dispatch(
            action,
            self.shared.now(),
            self.shared.settings.verification_ms,
        )?;
        if active
            .sender
            .try_send(QueuedCommand {
                sequence: receipt.sequence,
                action,
            })
            .is_err()
        {
            *model = previous;
            return Err("command_queue_unavailable".into());
        }
        drop(model);
        self.shared.state();
        Ok(receipt)
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
