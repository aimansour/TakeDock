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
                state["state"] = json!(match (key, state["state"].as_str().unwrap_or("idle")) {
                    ("up", "idle") => "recording",
                    ("up", _) => "idle",
                    ("down", "recording") => "paused",
                    ("down", "paused") => "recording",
                    _ => "idle",
                });
                fs::write(root.join("state.json"), state.to_string()).unwrap();
                let marker = line
                    .split("echo ")
                    .nth(1)
                    .unwrap()
                    .trim()
                    .replace("$?", "0");
                println!("{marker}");
                io::stdout().flush().unwrap();
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
                if root.join(format!("stop-{session}")).exists() || root.join("force-stop").exists()
                {
                    fs::write(root.join("observer-stopped"), b"").unwrap();
                    println!("INSTRUMENTATION_CODE: 0");
                    return;
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
        ["exec-out", command] => {
            let args = shell_words::split(command).unwrap();
            if !args
                .first()
                .is_some_and(|path| path.starts_with("/data/local/tmp/takedock-files-"))
            {
                std::process::exit(2)
            }
            if let Err(error) = file_command(&root, &args[1..]) {
                eprintln!("{error}");
                std::process::exit(1)
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
                    if root.join("slow-read").exists() {
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
        }
        _ => return Err("invalid_fixture_helper_command".into()),
    }
    Ok(())
}
struct SlowWriter<W: Write>(W);
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
