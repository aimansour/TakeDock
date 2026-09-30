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
        _ => {
            eprintln!("Unsupported fixture ADB invocation: {args:?}");
            std::process::exit(2);
        }
    }
}
