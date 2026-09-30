use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

pub fn read_tracking_frame(reader: &mut impl Read) -> Result<Vec<(String, String)>, String> {
    let mut header = [0; 4];
    reader
        .read_exact(&mut header)
        .map_err(|error| error.to_string())?;
    let size = usize::from_str_radix(
        std::str::from_utf8(&header).map_err(|error| error.to_string())?,
        16,
    )
    .map_err(|error| error.to_string())?;
    let mut body = vec![0; size];
    reader
        .read_exact(&mut body)
        .map_err(|error| error.to_string())?;
    let text = std::str::from_utf8(&body).map_err(|error| error.to_string())?;
    text.lines()
        .map(|line| {
            let mut parts = line.split_whitespace();
            Ok((
                parts.next().ok_or("missing_device")?.into(),
                parts.next().ok_or("missing_status")?.into(),
            ))
        })
        .collect()
}
pub fn parse_observer_line(line: &str) -> Result<Option<serde_json::Value>, String> {
    let Some(payload) = line.strip_prefix("INSTRUMENTATION_STATUS: takedock=") else {
        return Ok(None);
    };
    let value: serde_json::Value =
        serde_json::from_str(payload.trim()).map_err(|error| error.to_string())?;
    if value["version"] != 1 || !value["session"].is_string() || !value["type"].is_string() {
        return Err("unsupported_observer_protocol".into());
    }
    Ok(Some(value))
}
pub fn capture_command(sequence: u64, action: crate::model::RecordingAction) -> String {
    use crate::model::RecordingAction::*;
    let key = match action {
        Start | Stop => "KEYCODE_VOLUME_UP",
        Pause | Resume => "KEYCODE_VOLUME_DOWN",
    };
    format!("input keyevent --async {key}; echo TD_DONE_{sequence}_$?\n")
}

pub type ProcessHandle = Arc<Mutex<Child>>;

#[derive(Debug, Clone)]
pub struct Adb {
    pub path: PathBuf,
}
impl Adb {
    pub fn resolve(custom: &str) -> Result<Self, String> {
        let executable = if cfg!(windows) { "adb.exe" } else { "adb" };
        let mut candidates = Vec::new();
        if !custom.is_empty() {
            candidates.push(PathBuf::from(custom));
        } else {
            if let Some(paths) = std::env::var_os("PATH") {
                candidates.extend(std::env::split_paths(&paths).map(|path| path.join(executable)));
            }
            for name in ["ANDROID_HOME", "ANDROID_SDK_ROOT"] {
                if let Some(path) = std::env::var_os(name) {
                    candidates.push(PathBuf::from(path).join("platform-tools").join(executable));
                }
            }
            if let Some(path) = std::env::var_os("LOCALAPPDATA") {
                candidates.push(
                    PathBuf::from(path)
                        .join("Android/Sdk/platform-tools")
                        .join(executable),
                );
            }
        }
        let path = candidates
            .into_iter()
            .find(|path| path.is_file())
            .ok_or("adb_not_found")?;
        Ok(Self { path })
    }
    pub fn command(&self, serial: Option<&str>, arguments: &[&str]) -> Command {
        let mut command = Command::new(&self.path);
        if let Some(serial) = serial {
            command.args(["-s", serial]);
        }
        command
            .args(arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        command
    }
    pub fn spawn(
        &self,
        serial: Option<&str>,
        arguments: &[&str],
        interactive: bool,
    ) -> Result<Child, String> {
        let mut command = self.command(serial, arguments);
        if interactive {
            command.stdin(Stdio::piped());
        }
        command.spawn().map_err(|error| error.to_string())
    }
    pub fn output(
        &self,
        serial: &str,
        arguments: &[&str],
        timeout_ms: u64,
        cancelled: &std::sync::atomic::AtomicBool,
    ) -> Result<Vec<u8>, String> {
        use std::{
            sync::atomic::Ordering,
            thread,
            time::{Duration, Instant},
        };
        let mut child = self.spawn(Some(serial), arguments, false)?;
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let output = thread::spawn(move || {
            let mut bytes = Vec::new();
            let result = stdout.take(8 * 1024 * 1024 + 1).read_to_end(&mut bytes);
            (result, bytes)
        });
        let errors = thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = stderr.take(64 * 1024).read_to_end(&mut bytes);
            bytes
        });
        let start = Instant::now();
        let status = loop {
            if cancelled.load(Ordering::Acquire)
                || start.elapsed().as_millis() >= timeout_ms as u128
            {
                let _ = child.kill();
                let _ = child.wait();
                let _ = output.join();
                let _ = errors.join();
                return Err(if cancelled.load(Ordering::Acquire) {
                    "operation_cancelled"
                } else {
                    "adb_operation_timeout"
                }
                .into());
            }
            if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
                break status;
            }
            thread::sleep(Duration::from_millis(10));
        };
        let (read_result, bytes) = output.join().map_err(|_| "adb_output_thread")?;
        let error_bytes = errors.join().map_err(|_| "adb_error_thread")?;
        read_result.map_err(|error| error.to_string())?;
        if !status.success() {
            return Err(String::from_utf8_lossy(&error_bytes).trim().to_string());
        }
        if bytes.len() > 8 * 1024 * 1024 {
            return Err("adb_output_too_large".into());
        }
        Ok(bytes)
    }
}
pub fn kill(process: &ProcessHandle) {
    if let Ok(mut child) = process.lock() {
        let _ = child.kill();
        let _ = child.wait();
    }
}
pub fn is_absolute_file(path: &Path) -> bool {
    path.is_absolute() && path.is_file()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn device_tracking_is_length_framed_and_retains_authorization_status() {
        let mut stream = &b"001aalpha\tdevice\nbeta\toffline\n"[..];
        assert_eq!(
            read_tracking_frame(&mut stream).unwrap(),
            [
                ("alpha".into(), "device".into()),
                ("beta".into(), "offline".into())
            ]
        );
    }
    #[test]
    fn malformed_and_truncated_tracking_is_rejected() {
        assert!(read_tracking_frame(&mut &b"zzzz"[..]).is_err());
        assert!(read_tracking_frame(&mut &b"0008abc"[..]).is_err());
    }
    #[test]
    fn only_versioned_observer_status_lines_are_accepted() {
        assert!(
            parse_observer_line("INSTRUMENTATION_STATUS_CODE: 0")
                .unwrap()
                .is_none()
        );
        assert_eq!(parse_observer_line("INSTRUMENTATION_STATUS: takedock={\"version\":1,\"type\":\"ready\",\"session\":\"x\"}").unwrap().unwrap()["type"], "ready");
        assert!(parse_observer_line("INSTRUMENTATION_STATUS: takedock={\"version\":2}").is_err());
    }
    #[test]
    fn capture_uses_async_input_and_unique_completion_marker() {
        let command = capture_command(42, crate::model::RecordingAction::Pause);
        assert!(command.starts_with("input keyevent --async KEYCODE_VOLUME_DOWN;"));
        assert!(command.contains("TD_DONE_42_"));
        assert_eq!(command.matches("input keyevent").count(), 1);
    }
}
