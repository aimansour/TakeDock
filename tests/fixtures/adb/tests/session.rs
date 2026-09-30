use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use takedock_lib::{
    model::{RecordingAction, RecordingState, Settings},
    session::Engine,
};

fn wait_until(mut predicate: impl FnMut() -> bool) {
    let start = Instant::now();
    while !predicate() {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "condition was not reached"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn fixture() -> (tempfile::TempDir, String) {
    let directory = tempfile::tempdir().unwrap();
    let executable = directory
        .path()
        .join(if cfg!(windows) { "adb.exe" } else { "adb" });
    std::fs::copy(env!("CARGO_BIN_EXE_takedock-adb-fixture"), &executable).unwrap();
    std::fs::write(directory.path().join("observer.apk"), b"fixture-apk").unwrap();
    (directory, executable.to_str().unwrap().into())
}
#[test]
fn a_closed_capture_shell_removes_controls_even_when_observer_keeps_emitting() {
    let (directory, adb_path) = fixture();
    let errors = Arc::new(std::sync::Mutex::new(Vec::new()));
    let output = errors.clone();
    let engine = Engine::start(
        Settings {
            adb_path,
            ..Default::default()
        },
        directory.path().join("observer.apk"),
        Arc::new(move |event, payload| {
            if event == "operation-error" {
                output.lock().unwrap().push(payload)
            }
        }),
    )
    .unwrap();
    wait_until(|| engine.snapshot().predicted == RecordingState::Idle);
    std::fs::write(directory.path().join("close-shell"), b"").unwrap();
    engine.dispatch(RecordingAction::Start).unwrap();
    wait_until(|| {
        errors
            .lock()
            .unwrap()
            .iter()
            .any(|event| event["code"] == "command_shell_closed")
    });
    std::fs::write(
        directory.path().join("state.json"),
        "{\"state\":\"paused\",\"foreground\":true,\"video_mode\":true}",
    )
    .unwrap();
    wait_until(|| engine.snapshot().observed == RecordingState::Paused);
    assert_eq!(engine.snapshot().predicted, RecordingState::Unknown);
    assert!(!engine.snapshot().observer_ready);
    assert!(engine.dispatch(RecordingAction::Resume).is_err());
    assert!(engine.lease().is_ok());
    engine.shutdown();
}
#[test]
fn commands_reach_a_real_persistent_process_while_observation_is_held() {
    let (directory, adb_path) = fixture();
    let engine = Engine::start(
        Settings {
            adb_path,
            ..Default::default()
        },
        directory.path().join("observer.apk"),
        Arc::new(|_, _| {}),
    )
    .unwrap();
    wait_until(|| engine.snapshot().predicted == RecordingState::Idle);
    std::fs::write(directory.path().join("hold-observation"), b"").unwrap();
    engine.dispatch(RecordingAction::Start).unwrap();
    engine.dispatch(RecordingAction::Pause).unwrap();
    wait_until(|| {
        std::fs::read_to_string(directory.path().join("commands.jsonl"))
            .unwrap_or_default()
            .lines()
            .count()
            == 2
    });
    assert_eq!(engine.snapshot().predicted, RecordingState::Paused);
    assert_eq!(engine.snapshot().observed, RecordingState::Idle);
    assert_eq!(engine.snapshot().pending, 2);
    std::fs::remove_file(directory.path().join("hold-observation")).unwrap();
    wait_until(|| engine.snapshot().observed == RecordingState::Paused);
    engine.shutdown();
    wait_until(|| directory.path().join("observer-stopped").exists());
}
#[test]
fn multiple_devices_remove_the_session_instead_of_selecting_arbitrarily() {
    let (directory, adb_path) = fixture();
    let engine = Engine::start(
        Settings {
            adb_path,
            ..Default::default()
        },
        directory.path().join("observer.apk"),
        Arc::new(|_, _| {}),
    )
    .unwrap();
    wait_until(|| engine.snapshot().connected);
    let old_lease = engine.lease().unwrap();
    std::fs::write(
        directory.path().join("devices.txt"),
        "fixture-one\tdevice\nfixture-two\tdevice\n",
    )
    .unwrap();
    wait_until(|| engine.snapshot().condition == "multiple_devices");
    assert!(!engine.snapshot().connected);
    assert!(
        old_lease
            .output(&["shell", "pm", "path", "app.takedock.observer"], 1000)
            .is_err()
    );
    assert!(engine.dispatch(RecordingAction::Start).is_err());
    engine.shutdown();
}
#[test]
fn observer_failure_keeps_the_authorized_phone_available_for_files() {
    let (directory, adb_path) = fixture();
    std::fs::write(directory.path().join("fail-install"), b"").unwrap();
    let engine = Engine::start(
        Settings {
            adb_path,
            ..Default::default()
        },
        directory.path().join("observer.apk"),
        Arc::new(|_, _| {}),
    )
    .unwrap();
    wait_until(|| engine.snapshot().condition == "observer_setup_failed");
    assert!(engine.snapshot().connected);
    assert!(engine.lease().is_ok());
    assert!(engine.dispatch(RecordingAction::Start).is_err());
    engine.shutdown();
}
#[test]
fn shutdown_during_installation_cannot_start_an_observer_after_exit() {
    let (directory, adb_path) = fixture();
    std::fs::write(directory.path().join("hold-install"), b"").unwrap();
    let engine = Engine::start(
        Settings {
            adb_path,
            ..Default::default()
        },
        directory.path().join("observer.apk"),
        Arc::new(|_, _| {}),
    )
    .unwrap();
    wait_until(|| directory.path().join("install-started").exists());
    engine.shutdown();
    std::fs::remove_file(directory.path().join("hold-install")).unwrap();
    // Negative lifecycle assertion; not used as proof of dispatch latency.
    std::thread::sleep(Duration::from_millis(300));
    assert!(!directory.path().join("observer-started").exists());
}
#[test]
fn restarted_engines_have_newer_generations_than_retired_events() {
    let (first, first_adb) = fixture();
    let first_engine = Engine::start(
        Settings {
            adb_path: first_adb,
            ..Default::default()
        },
        first.path().join("observer.apk"),
        Arc::new(|_, _| {}),
    )
    .unwrap();
    wait_until(|| first_engine.snapshot().predicted == RecordingState::Idle);
    first_engine.shutdown();
    let retired_generation = first_engine.snapshot().generation;
    let (second, second_adb) = fixture();
    let second_engine = Engine::start(
        Settings {
            adb_path: second_adb,
            ..Default::default()
        },
        second.path().join("observer.apk"),
        Arc::new(|_, _| {}),
    )
    .unwrap();
    wait_until(|| second_engine.snapshot().predicted == RecordingState::Idle);
    assert!(second_engine.snapshot().generation > retired_generation);
    second_engine.shutdown();
}
#[test]
fn a_stale_interface_cannot_send_a_capture_to_the_current_phone() {
    let (directory, adb_path) = fixture();
    let engine = Engine::start(
        Settings {
            adb_path,
            ..Default::default()
        },
        directory.path().join("observer.apk"),
        Arc::new(|_, _| {}),
    )
    .unwrap();
    wait_until(|| engine.snapshot().predicted == RecordingState::Idle);
    assert!(
        engine
            .dispatch_for(RecordingAction::Start, engine.snapshot().generation - 1)
            .is_err()
    );
    assert_eq!(engine.snapshot().command_sequence, 0);
    engine.shutdown();
}
