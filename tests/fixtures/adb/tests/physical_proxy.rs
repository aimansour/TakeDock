mod common;
use common::{Fixture, movie};
use serde_json::json;
use std::{
    fs,
    io::Write,
    process::{Command, Stdio},
};

fn trial(fixture: &Fixture) -> std::process::Output {
    let identity = takedock_files::Root::open(&fixture.path().join("videos"))
        .unwrap()
        .stat("take.mp4")
        .unwrap();
    let opaque = serde_json::to_string(&identity).unwrap();
    let helper = format!("/data/local/tmp/takedock-files-{}", "a".repeat(64));
    let manifest = fixture.path().join("allowlist.json");
    fs::write(&manifest,json!({"version":1,"serial":"fixture-one","helper":helper,"pre_start_ms":1,"take_started_ms":2,"pre_start":[],"takes":[{"name":"take.mp4","identity":opaque}]}).to_string()).unwrap();
    let remote = shell_words::join([
        helper.as_str(),
        "delete-guarded",
        "take.mp4",
        opaque.as_str(),
        "op-1",
    ]);
    let mut child = Command::new(env!("CARGO_BIN_EXE_device-discard-proxy"))
        .args(["-s", "fixture-one", "shell", "-T", &remote])
        .env("TAKEDOCK_TRIAL_REAL_ADB", &fixture.lease.adb.path)
        .env("TAKEDOCK_TRIAL_ALLOWLIST", manifest)
        .env("TAKEDOCK_TRIAL_BACKUP_DIR", fixture.path().join("backup"))
        .env("TAKEDOCK_TRIAL_FFMPEG", &fixture.lease.adb.path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"{\"type\":\"permit\",\"operation_id\":\"op-1\",\"allow\":true}\n")
        .unwrap();
    child.wait_with_output().unwrap()
}
#[test]
fn successful_process_trial_preserves_verified_bytes_before_deletion() {
    let fixture = Fixture::new();
    let bytes = movie();
    fs::write(fixture.video("take.mp4"), &bytes).unwrap();
    let result = trial(&fixture);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(!fixture.video("take.mp4").exists());
    let entries = fs::read_dir(fixture.path().join("backup"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect::<Vec<_>>();
    let backup = entries
        .iter()
        .find(|p| p.extension().is_some_and(|e| e == "mp4"))
        .unwrap();
    assert_eq!(fs::read(backup).unwrap(), bytes);
    assert!(
        entries
            .iter()
            .any(|p| p.to_string_lossy().ends_with("receipt.json"))
    );
    assert!(fixture.path().join("decode-started").exists());
    let helper_calls = fs::read_to_string(fixture.path().join("helper-calls.jsonl")).unwrap();
    assert!(!helper_calls.lines().any(|line| {
        let args: Vec<String> = serde_json::from_str(line).unwrap();
        ["read", "hash"].contains(&args[0].as_str())
    }));
}
#[test]
fn failed_decode_or_read_never_starts_the_remote_permit_wait() {
    for failure in ["fail-decode", "fail-read", "bad-hash"] {
        let fixture = Fixture::new();
        let bytes = movie();
        fs::write(fixture.video("take.mp4"), &bytes).unwrap();
        fixture.flag(failure);
        let result = trial(&fixture);
        assert!(!result.status.success());
        assert_eq!(fs::read(fixture.video("take.mp4")).unwrap(), bytes);
        assert!(!fixture.path().join("delete-prepared").exists());
    }
}
