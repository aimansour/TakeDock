#![allow(dead_code)]
pub mod jsonl;
use std::{
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};
use takedock_lib::{adb::Adb, session::DeviceLease};

#[track_caller]
pub fn wait(mut predicate: impl FnMut() -> bool) {
    let start = Instant::now();
    while !predicate() {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "fixture condition was not reached"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
pub fn movie() -> Vec<u8> {
    fn atom(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut bytes = ((data.len() + 8) as u32).to_be_bytes().to_vec();
        bytes.extend(kind);
        bytes.extend(data);
        bytes
    }
    let mut timeline = [0; 20];
    timeline[12..16].copy_from_slice(&1000u32.to_be_bytes());
    timeline[16..20].copy_from_slice(&100u32.to_be_bytes());
    [
        atom(b"ftyp", b"isom0000"),
        atom(b"mdat", b"fixture"),
        atom(b"moov", &atom(b"mvhd", &timeline)),
    ]
    .concat()
}
pub struct Fixture {
    pub root: tempfile::TempDir,
    pub lease: DeviceLease,
}
impl Fixture {
    pub fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let executable = root
            .path()
            .join(if cfg!(windows) { "adb.exe" } else { "adb" });
        std::fs::copy(env!("CARGO_BIN_EXE_takedock-adb-fixture"), &executable).unwrap();
        std::fs::create_dir(root.path().join("videos")).unwrap();
        std::fs::create_dir(root.path().join("destination")).unwrap();
        std::fs::create_dir_all(root.path().join("file-helper/arm64-v8a")).unwrap();
        std::fs::write(
            root.path().join("file-helper/arm64-v8a/takedock-files"),
            b"fixture-helper",
        )
        .unwrap();
        std::fs::write(root.path().join("observer.apk"), b"fixture-apk").unwrap();
        Self {
            lease: DeviceLease {
                adb: Adb { path: executable },
                serial: "fixture-one".into(),
                generation: 7,
                cancelled: Arc::new(AtomicBool::new(false)),
            },
            root,
        }
    }
    pub fn path(&self) -> &Path {
        self.root.path()
    }
    pub fn video(&self, name: &str) -> PathBuf {
        self.path().join("videos").join(name)
    }
    pub fn flag(&self, name: &str) {
        std::fs::write(self.path().join(name), b"").unwrap();
    }
    pub fn unflag(&self, name: &str) {
        let _ = std::fs::remove_file(self.path().join(name));
    }
}
pub struct ChildGuard(pub std::process::Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
pub fn alive(pid: u32) -> bool {
    #[cfg(windows)]
    {
        String::from_utf8_lossy(
            &std::process::Command::new("tasklist")
                .args(["/FO", "CSV", "/NH", "/FI", &format!("PID eq {pid}")])
                .output()
                .unwrap()
                .stdout,
        )
        .contains(&format!("\"{pid}\""))
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap()
            .success()
    }
}
