//! External peer fixture. No real ADB forwarding and no application test hooks.
use serde_json::{Value, json};
use std::{
    fs,
    io::{self, BufRead, Write},
    path::{Path, PathBuf},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
fn root() -> PathBuf {
    std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}
fn read_state(root: &Path) -> Value {
    fs::read(root.join("state.json"))
        .ok()
        .and_then(|data| serde_json::from_slice(&data).ok())
        .unwrap_or(json!({"state":"idle","foreground":true,"video_mode":true}))
}
fn emit(session: &str, mut value: Value) {
    value["version"] = json!(1);
    value["session"] = json!(session);
    println!("INSTRUMENTATION_STATUS: takedock={value}\nINSTRUMENTATION_STATUS_CODE: 0");
    io::stdout().flush().unwrap();
}
fn main() {
    let root = root();
    let mut arguments = std::env::args().skip(1).collect::<Vec<_>>();
    if arguments.first().is_some_and(|argument| argument == "-s") {
        if arguments
            .get(1)
            .is_none_or(|serial| !serial.starts_with("fixture-"))
        {
            std::process::exit(2);
        }
        arguments.drain(..2);
    }
    let args = arguments.iter().map(String::as_str).collect::<Vec<_>>();
    match args.as_slice() {
        [
            "-nostdin",
            "-v",
            "error",
            "-xerror",
            "-i",
            _,
            "-f",
            "null",
            "-",
        ] => {
            fs::write(root.join("decode-started"), b"").unwrap();
            if root.join("fail-decode").exists() {
                std::process::exit(1);
            }
        }
        ["track-devices"] => {
            let mut previous = String::new();
            loop {
                let devices = fs::read_to_string(root.join("devices.txt"))
                    .unwrap_or("fixture-one\tdevice\n".into());
                if previous != devices {
                    print!("{:04x}{}", devices.len(), devices);
                    io::stdout().flush().unwrap();
                    previous = devices;
                }
                thread::sleep(Duration::from_millis(10));
            }
        }
        ["shell", "-T"] => {
            for line in io::stdin().lock().lines() {
                let line = line.unwrap();
                if line == "exit" {
                    return;
                }
                let key = if line.starts_with("input keyevent --async KEYCODE_VOLUME_UP;") {
                    "up"
                } else if line.starts_with("input keyevent --async KEYCODE_VOLUME_DOWN;") {
                    "down"
                } else {
                    std::process::exit(3);
                };
                let mut trace = fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(root.join("commands.jsonl"))
                    .unwrap();
                writeln!(trace, "{}", json!({"key":key,"command":line,"time":SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis()})).unwrap();
                let mut state = read_state(&root);
                let previous_state = state["state"].as_str().unwrap_or("idle").to_owned();
                state["state"] = json!(match (key, state["state"].as_str().unwrap_or("idle")) {
                    ("up", "idle") => "recording",
                    ("up", _) => "idle",
                    ("down", "recording") => "paused",
                    ("down", "paused") => "recording",
                    _ => "idle",
                });
                if key == "up"
                    && previous_state != "idle"
                    && root.join("ignore-early-stop").exists()
                {
                    let start: u128 = fs::read_to_string(root.join("take-start-ms"))
                        .ok()
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(0);
                    if SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_millis()
                        .saturating_sub(start)
                        < 500
                    {
                        state["state"] = json!(previous_state);
                    }
                }
                if root.join("record-take-files").exists()
                    && key == "up"
                    && previous_state == "idle"
                    && state["state"] == "recording"
                {
                    let counter: u64 = fs::read_to_string(root.join("take-counter"))
                        .ok()
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(0)
                        + 1;
                    let name = format!("take-{counter:04}.mp4");
                    fs::write(root.join("take-counter"), counter.to_string()).unwrap();
                    fs::write(root.join("current-take-name"), &name).unwrap();
                    fs::write(
                        root.join("take-start-ms"),
                        SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_millis()
                            .to_string(),
                    )
                    .unwrap();
                    fs::write(root.join("videos").join(name), b"fixture-recording").unwrap();
                    fs::write(root.join("take-created"), b"").unwrap();
                }
                fs::write(root.join("state.tmp"), state.to_string()).unwrap();
                fs::rename(root.join("state.tmp"), root.join("state.json")).unwrap();
                finish_fixture_take(&root);
                let marker = line
                    .split("echo ")
                    .nth(1)
                    .unwrap()
                    .trim()
                    .replace("$?", "0");
                println!("{marker}");
                io::stdout().flush().unwrap();
                if root.join("close-shell").exists() {
                    return;
                }
            }
        }
        [
            "shell",
            "am",
            "instrument",
            "-w",
            "-r",
            "-e",
            "session",
            session,
            "app.takedock.observer/.ObserverInstrumentation",
        ] => {
            let _ = fs::remove_file(root.join("force-stop"));
            fs::write(root.join("observer-started"), session).unwrap();
            emit(session, json!({"type":"ready"}));
            let mut previous = Value::Null;
            let mut sequence = 0u64;
            loop {
                finish_fixture_take(&root);
                if root.join(format!("stop-{session}")).exists() || root.join("force-stop").exists()
                {
                    fs::write(root.join("observer-stopped"), b"").unwrap();
                    println!("INSTRUMENTATION_CODE: 0");
                    return;
                }
                if let Ok(raw) = fs::read(root.join("inject-event.json")) {
                    let event: Value = serde_json::from_slice(&raw).unwrap();
                    let injected_session = event["session"].as_str().unwrap_or(session).to_owned();
                    emit(&injected_session, event);
                    fs::remove_file(root.join("inject-event.json")).unwrap();
                    fs::write(root.join("injected-event"), b"").unwrap();
                }
                if !root.join("hold-observation").exists() {
                    let state = read_state(&root);
                    if state != previous {
                        sequence += 1;
                        let mut event = state.clone();
                        event["type"] = json!("state");
                        event["seq"] = json!(sequence);
                        event["event_time"] = json!(sequence * 100);
                        emit(session, event);
                        previous = state;
                    }
                }
                thread::sleep(Duration::from_millis(10));
            }
        }
        ["shell", "pm", "path", "app.takedock.observer"] => {}
        ["install", "-r", path] if Path::new(path).is_file() => {
            fs::write(root.join("install-started"), b"").unwrap();
            while root.join("hold-install").exists() {
                thread::sleep(Duration::from_millis(10));
            }
            if root.join("fail-install").exists() {
                eprintln!("fixture_install_failure");
                std::process::exit(1);
            }
            println!("Success");
        }
        [
            "shell",
            "am",
            "broadcast",
            "-a",
            "app.takedock.observer.STOP",
            "-p",
            "app.takedock.observer",
            "--es",
            "session",
            session,
        ] => {
            fs::write(root.join(format!("stop-{session}")), b"").unwrap();
            println!("Broadcast completed: result=0");
        }
        ["shell", "am", "force-stop", "app.takedock.observer"] => {
            fs::write(root.join("force-stop"), b"").unwrap();
        }
        ["shell", "getprop", "ro.product.cpu.abi"] => println!("arm64-v8a"),
        ["shell", "sha256sum", path] => {
            use sha2::{Digest, Sha256};
            let file = root
                .join("device")
                .join(Path::new(path).file_name().unwrap());
            match fs::read(file) {
                Ok(bytes) => println!(
                    "{}  {path}",
                    Sha256::digest(bytes)
                        .iter()
                        .map(|b| format!("{b:02x}"))
                        .collect::<String>()
                ),
                Err(_) => std::process::exit(1),
            }
        }
        ["push", from, to] => {
            fs::write(root.join("helper-push-started"), b"").unwrap();
            fs::write(root.join("helper-push-pid"), std::process::id().to_string()).unwrap();
            while root.join("hold-helper-push").exists() {
                thread::sleep(Duration::from_millis(10));
            }
            fs::create_dir_all(root.join("device")).unwrap();
            fs::copy(
                from,
                root.join("device").join(Path::new(to).file_name().unwrap()),
            )
            .unwrap();
        }
        ["shell", "chmod", "700", _] => {}
        [
            "shell",
            "am",
            "broadcast",
            "-a",
            "android.intent.action.MEDIA_SCANNER_SCAN_FILE",
            "-d",
            uri,
        ] if uri.starts_with("file:///sdcard/DCIM/OpenCamera/") => {
            fs::write(root.join("index-started"), b"").unwrap();
            while root.join("hold-index").exists() {
                thread::sleep(Duration::from_millis(10));
            }
            println!("Broadcast completed: result=0");
        }
        ["shell", "mv", "-f", from, to] => {
            fs::rename(
                root.join("device")
                    .join(Path::new(from).file_name().unwrap()),
                root.join("device").join(Path::new(to).file_name().unwrap()),
            )
            .unwrap();
        }
        ["shell", "-T", command] => {
            let args = shell_words::split(command).unwrap();
            if !args
                .first()
                .is_some_and(|path| path.starts_with("/data/local/tmp/takedock-files-"))
            {
                std::process::exit(2);
            }
            if let Err(error) = file_command(&root, &args[1..]) {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
        ["exec-out", command] => {
            let args = shell_words::split(command).unwrap();
            if !args
                .first()
                .is_some_and(|path| path.starts_with("/data/local/tmp/takedock-files-"))
            {
                std::process::exit(2)
            }
            if let Err(error) = file_command(&root, &args[1..]) {
                // Legacy exec transport merges remote output and does not
                // report the remote exit code. Only shell -T does that.
                println!("{error}");
            }
        }
        _ => {
            eprintln!("Unsupported fixture ADB invocation: {args:?}");
            std::process::exit(2);
        }
    }
}
fn file_command(root: &Path, args: &[String]) -> Result<(), String> {
    let videos = takedock_files::Root::open(&root.join("videos"))?;
    let borrowed = args.iter().map(String::as_str).collect::<Vec<_>>();
    match borrowed.as_slice() {
        ["watch", stream] => return watch_files(root, &videos, stream),
        ["inspect", name] => {
            if root.join("fail-rename-inspect").exists()
                && fs::read_to_string(root.join("rename-acknowledged"))
                    .ok()
                    .as_deref()
                    == Some(name)
            {
                return Err("fixture_rename_inspection_failure".into());
            }
            println!("{}", serde_json::to_string(&videos.inspect(name)?).unwrap());
            return Ok(());
        }
        _ => {}
    }
    let _lock = videos.lock()?;
    videos.recover()?;
    let args = args.iter().map(String::as_str).collect::<Vec<_>>();
    match args.as_slice() {
        ["list"] => {
            for entry in videos.list()? {
                let mut output = io::stdout().lock();
                output.write_all(entry.name.as_bytes()).unwrap();
                output.write_all(&[0]).unwrap();
                output
                    .write_all(&serde_json::to_vec(&entry).unwrap())
                    .unwrap();
                output.write_all(&[0]).unwrap();
            }
        }
        ["stat", name] => println!("{}", serde_json::to_string(&videos.stat(name)?).unwrap()),
        ["delete-guarded", name, expected, operation] => {
            let mut trace = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(root.join("delete-calls.jsonl"))
                .unwrap();
            writeln!(
                trace,
                "{}",
                json!({"operation":operation,"name":name,"identity":expected})
            )
            .unwrap();
            fs::write(root.join("delete-prepared"), name).unwrap();
            while root.join("hold-delete-prepared").exists() {
                thread::sleep(Duration::from_millis(10));
            }
            let expected = serde_json::from_str(expected).map_err(|e| e.to_string())?;
            takedock_files::guarded::serve(
                &videos,
                name,
                &expected,
                operation,
                takedock_files::guarded::read_permit_stdin,
                &mut GuardOutput {
                    root,
                    bytes: Vec::new(),
                },
            )?;
        }
        [kind, name, expected] if ["read", "hash", "delete"].contains(kind) => {
            let identity = serde_json::from_str(expected).map_err(|e| e.to_string())?;
            if !videos.video_ready(name)? {
                return Err("video_not_finalized".into());
            }
            match *kind {
                "read" => {
                    fs::write(root.join("read-started"), b"").unwrap();
                    if root.join("fail-read").exists() {
                        return Err("fixture_transfer_failure".into());
                    }
                    if root.join("hold-read-tail").exists() {
                        let mut held = HeldTailWriter {
                            output: SlowWriter(io::stdout().lock()),
                            root: root.to_path_buf(),
                            written: 0,
                        };
                        videos.read(name, &identity, &mut held)?;
                    } else if root.join("slow-read").exists() {
                        let mut slow = SlowWriter(io::stdout().lock());
                        videos.read(name, &identity, &mut slow)?;
                    } else {
                        videos.read(name, &identity, &mut io::stdout().lock())?;
                    }
                }
                "hash" => {
                    fs::write(root.join("hash-started"), b"").unwrap();
                    while root.join("hold-hash").exists() {
                        thread::sleep(Duration::from_millis(10));
                    }
                    println!(
                        "{}",
                        if root.join("bad-hash").exists() {
                            "0".repeat(64)
                        } else {
                            videos.hash(name, &identity)?
                        }
                    );
                }
                "delete" => {
                    fs::write(root.join("delete-called"), name).unwrap();
                    videos.delete(name, &identity)?;
                }
                _ => unreachable!(),
            }
        }
        ["rename", name, new_name, expected] => {
            if !videos.video_ready(name)? {
                return Err("video_not_finalized".into());
            }
            videos.rename(
                name,
                new_name,
                &serde_json::from_str(expected).map_err(|e| e.to_string())?,
            )?;
            fs::write(root.join("rename-acknowledged"), new_name).map_err(|e| e.to_string())?;
            while root.join("hold-rename-ack").exists() {
                thread::sleep(Duration::from_millis(10));
            }
        }
        _ => return Err("invalid_fixture_helper_command".into()),
    }
    Ok(())
}
fn finish_fixture_take(root: &Path) {
    if !root.join("record-take-files").exists()
        || root.join("hold-finalization").exists()
        || read_state(root)["state"] != "idle"
    {
        return;
    }
    let Ok(name) = fs::read_to_string(root.join("current-take-name")) else {
        return;
    };
    let path = root.join("videos").join(name);
    if fs::read(&path).ok().as_deref() != Some(b"fixture-recording") {
        return;
    }
    fn atom(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut bytes = ((data.len() + 8) as u32).to_be_bytes().to_vec();
        bytes.extend(kind);
        bytes.extend(data);
        bytes
    }
    let mut timeline = [0; 20];
    timeline[12..16].copy_from_slice(&1000u32.to_be_bytes());
    timeline[16..20].copy_from_slice(&100u32.to_be_bytes());
    fs::write(
        path,
        [
            atom(b"ftyp", b"isom0000"),
            atom(b"mdat", b"capture"),
            atom(b"moov", &atom(b"mvhd", &timeline)),
        ]
        .concat(),
    )
    .unwrap();
}

struct GuardOutput<'a> {
    root: &'a Path,
    bytes: Vec<u8>,
}
impl Write for GuardOutput<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.bytes.extend(bytes);
        while let Some(end) = self.bytes.iter().position(|byte| *byte == b'\n') {
            let line: Vec<_> = self.bytes.drain(..=end).collect();
            let message: Value = serde_json::from_slice(&line).unwrap();
            if message["type"] == "result" && message["status"] == "deleted" {
                fs::write(self.root.join("delete-result-held"), b"").unwrap();
                while self.root.join("hold-delete-result").exists() {
                    thread::sleep(Duration::from_millis(10));
                }
                if self.root.join("drop-delete-result").exists() {
                    return Err(io::ErrorKind::BrokenPipe.into());
                }
            }
            io::stdout().lock().write_all(&line)?;
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        io::stdout().flush()
    }
}

fn watch_files(root: &Path, videos: &takedock_files::Root, stream: &str) -> Result<(), String> {
    use std::{collections::BTreeMap, time::Instant};
    use takedock_files::watch::{ManifestEntry, WatchBody, WatchFrame};
    fs::write(root.join("watch-pid"), std::process::id().to_string()).map_err(|e| e.to_string())?;
    fs::write(root.join("watch-started"), b"").map_err(|e| e.to_string())?;
    if root.join("malformed-watch").exists() {
        println!("{{\"version\":7}}");
        io::stdout().flush().unwrap();
        return Ok(());
    }
    while root.join("hold-watch-baseline").exists() {
        thread::sleep(Duration::from_millis(10));
    }
    let mut sequence = 0;
    let mut send = |body| -> Result<(), String> {
        sequence += 1;
        let frame = WatchFrame {
            version: 1,
            stream: stream.into(),
            sequence,
            body,
        };
        let mut output = io::stdout().lock();
        serde_json::to_writer(&mut output, &frame).map_err(|e| e.to_string())?;
        output
            .write_all(b"\n")
            .and_then(|_| output.flush())
            .map_err(|e| e.to_string())
    };
    send(WatchBody::BaselineBegin)?;
    let mut previous = BTreeMap::new();
    for entry in videos.manifest()? {
        previous.insert(entry.name.clone(), entry.identity.clone());
        send(WatchBody::BaselineEntry { entry })?;
    }
    send(WatchBody::BaselineEnd)?;
    let mut heartbeat = Instant::now();
    loop {
        if root.join("watch-gap").exists() {
            let frame = WatchFrame {
                version: 1,
                stream: stream.into(),
                sequence: u64::MAX,
                body: WatchBody::Heartbeat,
            };
            println!("{}", serde_json::to_string(&frame).unwrap());
            io::stdout().flush().unwrap();
            return Ok(());
        }
        let current = videos
            .manifest()?
            .into_iter()
            .map(|entry| (entry.name, entry.identity))
            .collect::<BTreeMap<_, _>>();
        for (name, identity) in &current {
            let entry = ManifestEntry {
                name: name.clone(),
                identity: identity.clone(),
            };
            if !previous.contains_key(name) {
                send(WatchBody::Created {
                    entry: entry.clone(),
                })?;
                send(WatchBody::Closed { entry })?;
            } else if previous.get(name) != Some(identity) {
                send(WatchBody::Changed {
                    entry: entry.clone(),
                })?;
                send(WatchBody::Closed { entry })?;
            }
        }
        for name in previous.keys().filter(|name| !current.contains_key(*name)) {
            send(WatchBody::Removed { name: name.clone() })?;
        }
        previous = current;
        if heartbeat.elapsed() >= Duration::from_millis(1000)
            && !root.join("stop-watch-heartbeats").exists()
        {
            send(WatchBody::Heartbeat)?;
            heartbeat = Instant::now();
        }
        thread::sleep(Duration::from_millis(25));
    }
}
struct SlowWriter<W: Write>(W);
struct HeldTailWriter<W: Write> {
    output: SlowWriter<W>,
    root: PathBuf,
    written: usize,
}
impl<W: Write> Write for HeldTailWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.written >= 64 * 1024 {
            fs::write(self.root.join("read-prefix-held"), b"")?;
            while self.root.join("hold-read-tail").exists() {
                thread::sleep(Duration::from_millis(10));
            }
        }
        let limit = if self.written < 64 * 1024 {
            bytes.len().min(64 * 1024 - self.written)
        } else {
            bytes.len()
        };
        let size = self.output.write(&bytes[..limit])?;
        self.written += size;
        Ok(size)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.output.flush()
    }
}
impl<W: Write> Write for SlowWriter<W> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        thread::sleep(Duration::from_millis(20));
        let size = buffer.len().min(4096);
        self.0.write(&buffer[..size])
    }
    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}
