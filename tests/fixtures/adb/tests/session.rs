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
    let cues = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let output = cues.clone();
    let engine = Engine::start(
        Settings {
            adb_path,
            ..Default::default()
        },
        directory.path().join("observer.apk"),
        Arc::new(move |event, _| {
            if event == "speech-ready" {
                output.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        }),
    )
    .unwrap();
    wait_until(|| engine.snapshot().predicted == RecordingState::Idle);
    std::fs::write(directory.path().join("hold-observation"), b"").unwrap();
    engine.dispatch(RecordingAction::Start).unwrap();
    engine.dispatch(RecordingAction::Pause).unwrap();
    engine.dispatch(RecordingAction::Resume).unwrap();
    engine.dispatch(RecordingAction::Stop).unwrap();
    wait_until(|| {
        std::fs::read_to_string(directory.path().join("commands.jsonl"))
            .unwrap_or_default()
            .lines()
            .count()
            == 4
    });
    assert_eq!(engine.snapshot().predicted, RecordingState::Idle);
    assert_eq!(engine.snapshot().observed, RecordingState::Idle);
    assert_eq!(engine.snapshot().pending, 4);
    assert_eq!(
        engine.snapshot().speech_readiness,
        takedock_lib::model::SpeechReadiness::Inactive
    );
    assert_eq!(cues.load(std::sync::atomic::Ordering::SeqCst), 0);
    std::fs::remove_file(directory.path().join("hold-observation")).unwrap();
    // The final idle state is unchanged from the Observer's previous idle;
    // ordered evidence is intentionally still held/unconfirmed.
    engine.shutdown();
    wait_until(|| directory.path().join("observer-stopped").exists());
}

#[test]
fn confirmed_start_emits_one_cue_and_retired_engine_cannot_claim_it() {
    use takedock_lib::{model::SpeechReadiness, readiness::ReadyToken};
    let (directory, adb_path) = fixture();
    let cues = Arc::new(std::sync::Mutex::new(Vec::<ReadyToken>::new()));
    let output = cues.clone();
    let engine = Engine::start(
        Settings {
            adb_path,
            ready_delay_ms: 500,
            ..Default::default()
        },
        directory.path().join("observer.apk"),
        Arc::new(move |event, payload| {
            if event == "speech-ready" {
                output
                    .lock()
                    .unwrap()
                    .push(serde_json::from_value(payload).unwrap());
            }
        }),
    )
    .unwrap();
    wait_until(|| engine.snapshot().predicted == RecordingState::Idle);
    engine.dispatch(RecordingAction::Start).unwrap();
    wait_until(|| !cues.lock().unwrap().is_empty());
    assert_eq!(engine.snapshot().speech_readiness, SpeechReadiness::Ready);
    let token = cues.lock().unwrap()[0].clone();
    assert!(engine.claim_ready_cue(&token));
    assert!(!engine.claim_ready_cue(&token));
    engine.dispatch(RecordingAction::Stop).unwrap();
    wait_until(|| engine.snapshot().observed == RecordingState::Idle);
    engine.dispatch(RecordingAction::Start).unwrap();
    wait_until(|| cues.lock().unwrap().len() == 2);
    let unclaimed = cues.lock().unwrap()[1].clone();
    assert_ne!(token.id, unclaimed.id);
    engine.shutdown();
    assert_eq!(cues.lock().unwrap().len(), 2);
    assert!(!engine.claim_ready_cue(&unclaimed));
    let next = Engine::start(
        Settings {
            adb_path: directory
                .path()
                .join(if cfg!(windows) { "adb.exe" } else { "adb" })
                .to_str()
                .unwrap()
                .into(),
            ..Default::default()
        },
        directory.path().join("observer.apk"),
        Arc::new(|_, _| {}),
    )
    .unwrap();
    wait_until(|| next.snapshot().predicted == RecordingState::Recording);
    assert_eq!(next.snapshot().speech_readiness, SpeechReadiness::Inactive);
    assert!(!next.claim_ready_cue(&unclaimed));
    next.dispatch(RecordingAction::Stop).unwrap();
    wait_until(|| next.snapshot().observed == RecordingState::Idle);
    next.shutdown();
}

#[test]
fn reconnect_keeps_updated_policy_without_replaying_an_existing_recording() {
    use takedock_lib::{model::SpeechReadiness, readiness::ReadinessPolicy};
    let (directory, adb_path) = fixture();
    let cues = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let output = cues.clone();
    let engine = Engine::start(
        Settings {
            adb_path,
            ..Default::default()
        },
        directory.path().join("observer.apk"),
        Arc::new(move |event, _| {
            if event == "speech-ready" {
                output.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        }),
    )
    .unwrap();
    wait_until(|| engine.snapshot().predicted == RecordingState::Idle);
    engine.set_readiness_policy(ReadinessPolicy {
        sound: false,
        delay_ms: 500,
    });
    engine.dispatch(RecordingAction::Start).unwrap();
    wait_until(|| engine.snapshot().speech_readiness == SpeechReadiness::Ready);
    let generation = engine.snapshot().generation;
    std::fs::write(directory.path().join("devices.txt"), "").unwrap();
    wait_until(|| !engine.snapshot().connected);
    std::fs::write(
        directory.path().join("devices.txt"),
        "fixture-one\tdevice\n",
    )
    .unwrap();
    wait_until(|| engine.snapshot().predicted == RecordingState::Recording);
    assert!(engine.snapshot().generation > generation);
    assert_eq!(
        engine.snapshot().speech_readiness,
        SpeechReadiness::Inactive
    );
    engine.dispatch(RecordingAction::Stop).unwrap();
    wait_until(|| engine.snapshot().observed == RecordingState::Idle);
    engine.dispatch(RecordingAction::Start).unwrap();
    wait_until(|| engine.snapshot().speech_readiness == SpeechReadiness::Ready);
    engine.shutdown();
    assert_eq!(cues.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[test]
fn ready_settings_apply_without_reconnect() {
    use takedock_lib::{
        model::SpeechReadiness,
        readiness::{ReadinessPolicy, ReadyToken},
    };
    let (directory, adb_path) = fixture();
    let cues = Arc::new(std::sync::Mutex::new(Vec::<ReadyToken>::new()));
    let output = cues.clone();
    let engine = Engine::start(
        Settings {
            adb_path,
            ..Default::default()
        },
        directory.path().join("observer.apk"),
        Arc::new(move |event, payload| {
            if event == "speech-ready" {
                output
                    .lock()
                    .unwrap()
                    .push(serde_json::from_value(payload).unwrap());
            }
        }),
    )
    .unwrap();
    wait_until(|| engine.snapshot().predicted == RecordingState::Idle);
    let generation = engine.snapshot().generation;
    engine.dispatch(RecordingAction::Start).unwrap();
    wait_until(|| engine.snapshot().observed == RecordingState::Recording);
    engine.set_readiness_policy(ReadinessPolicy {
        sound: false,
        delay_ms: 500,
    });
    engine.set_readiness_policy(ReadinessPolicy {
        sound: true,
        delay_ms: 500,
    });
    wait_until(|| engine.snapshot().speech_readiness == SpeechReadiness::Ready);
    assert!(cues.lock().unwrap().is_empty());
    engine.dispatch(RecordingAction::Stop).unwrap();
    wait_until(|| engine.snapshot().observed == RecordingState::Idle);
    engine.dispatch(RecordingAction::Start).unwrap();
    wait_until(|| !cues.lock().unwrap().is_empty());
    assert_eq!(engine.snapshot().generation, generation);
    assert_eq!(cues.lock().unwrap()[0].start_sequence, 3);
    engine.shutdown();
}

#[test]
fn held_audio_output_does_not_delay_capture_delivery() {
    use std::sync::{Mutex, Weak, mpsc};
    use takedock_lib::{
        audio::{AudioDispatcher, Tone},
        readiness::ReadyToken,
    };
    let (directory, adb_path) = fixture();
    let engine_slot: Arc<Mutex<Option<Weak<Engine>>>> = Arc::new(Mutex::new(None));
    let claim_slot = engine_slot.clone();
    let (played, tones) = mpsc::channel();
    let (release, held) = mpsc::channel();
    let held = Mutex::new(held);
    let first = std::sync::atomic::AtomicBool::new(true);
    let audio = Arc::new(AudioDispatcher::start(
        Arc::new(move |tone| {
            played.send(tone).unwrap();
            if first.swap(false, std::sync::atomic::Ordering::SeqCst) {
                held.lock()
                    .unwrap()
                    .recv_timeout(Duration::from_secs(5))
                    .map_err(|e| e.to_string())?;
            }
            Ok(())
        }),
        Arc::new(move |token| {
            let engine = claim_slot.lock().unwrap().as_ref().and_then(Weak::upgrade);
            engine.is_some_and(|engine| engine.claim_ready_cue(token))
        }),
        Arc::new(|_, error| panic!("unexpected audio error: {error}")),
    ));
    let queued = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let ready_queued = queued.clone();
    let output = audio.clone();
    let engine = Arc::new(
        Engine::start(
            Settings {
                adb_path,
                ready_delay_ms: 500,
                ..Default::default()
            },
            directory.path().join("observer.apk"),
            Arc::new(move |event, payload| {
                if event == "speech-ready" {
                    output.ready(serde_json::from_value::<ReadyToken>(payload).unwrap());
                    ready_queued.store(true, std::sync::atomic::Ordering::SeqCst);
                }
            }),
        )
        .unwrap(),
    );
    *engine_slot.lock().unwrap() = Some(Arc::downgrade(&engine));
    wait_until(|| engine.snapshot().predicted == RecordingState::Idle);
    audio.feedback(false);
    assert_eq!(
        tones.recv_timeout(Duration::from_secs(2)).unwrap(),
        Tone::Failure
    );
    engine.dispatch(RecordingAction::Start).unwrap();
    wait_until(|| queued.load(std::sync::atomic::Ordering::SeqCst));
    for action in [
        RecordingAction::Pause,
        RecordingAction::Resume,
        RecordingAction::Stop,
    ] {
        engine.dispatch(action).unwrap();
    }
    wait_until(|| {
        std::fs::read_to_string(directory.path().join("commands.jsonl"))
            .unwrap_or_default()
            .lines()
            .count()
            == 4
    });
    assert_eq!(engine.snapshot().predicted, RecordingState::Idle);
    audio.feedback(true);
    release.send(()).unwrap();
    assert_eq!(
        tones.recv_timeout(Duration::from_secs(2)).unwrap(),
        Tone::Success
    );
    engine.shutdown();
    audio.shutdown();
    assert!(tones.try_recv().is_err());
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

#[test]
fn observer_failures_cancel_confirmed_readiness_without_reviving_on_recovery() {
    use std::sync::Mutex;
    use takedock_lib::model::SpeechReadiness;
    for (message, code) in [
        (
            serde_json::json!({"type":"error","message":"State observation failed: IllegalStateException"}),
            "observer_failed",
        ),
        (
            serde_json::json!({"type":"state","seq":"invalid"}),
            "observer_protocol_invalid",
        ),
        (
            serde_json::json!({"type":null}),
            "observer_protocol_invalid",
        ),
    ] {
        let (directory, adb_path) = fixture();
        let events = Arc::new(Mutex::new(Vec::new()));
        let output = events.clone();
        let engine = Engine::start(
            Settings {
                adb_path,
                ready_delay_ms: 1000,
                ..Default::default()
            },
            directory.path().join("observer.apk"),
            Arc::new(move |event, value| output.lock().unwrap().push((event.to_owned(), value))),
        )
        .unwrap();
        wait_until(|| engine.snapshot().predicted == RecordingState::Idle);
        engine.dispatch(RecordingAction::Start).unwrap();
        wait_until(|| {
            engine.snapshot().observed == RecordingState::Recording
                && engine.snapshot().pending == 0
        });
        assert_eq!(
            engine.snapshot().speech_readiness,
            SpeechReadiness::Preparing
        );
        std::fs::write(directory.path().join("hold-observation"), b"").unwrap();
        inject_observer_event(directory.path(), &message);
        wait_until(|| {
            events
                .lock()
                .unwrap()
                .iter()
                .any(|(event, value)| event == "operation-error" && value["code"] == code)
        });
        assert_eq!(
            engine.snapshot().speech_readiness,
            SpeechReadiness::Inactive,
            "{code} must cancel the confirmed candidate"
        );
        assert!(
            engine.lease().is_ok(),
            "file operations retain the authorized device"
        );
        inject_observer_event(
            directory.path(),
            &serde_json::json!({"type":"state","seq":100,"event_time":10000,"state":"recording","foreground":true,"video_mode":true}),
        );
        std::thread::sleep(Duration::from_millis(1200));
        assert_eq!(
            engine.snapshot().speech_readiness,
            SpeechReadiness::Inactive
        );
        assert!(
            !events
                .lock()
                .unwrap()
                .iter()
                .any(|(event, _)| event == "speech-ready")
        );
        assert!(
            events
                .lock()
                .unwrap()
                .iter()
                .any(|(event, value)| event == "session-state"
                    && value["speech_readiness"] == "inactive"
                    && value["command_sequence"] == 1)
        );
        engine.shutdown();
    }
}

fn inject_observer_event(root: &std::path::Path, value: &serde_json::Value) {
    let _ = std::fs::remove_file(root.join("injected-event"));
    std::fs::write(
        root.join("inject-event.tmp"),
        serde_json::to_vec(value).unwrap(),
    )
    .unwrap();
    std::fs::rename(
        root.join("inject-event.tmp"),
        root.join("inject-event.json"),
    )
    .unwrap();
    wait_until(|| root.join("injected-event").exists());
}

#[test]
fn observer_failure_cancels_an_unclaimed_cue_behind_held_audio() {
    use std::sync::{Mutex, Weak, mpsc};
    use takedock_lib::{
        audio::{AudioDispatcher, Tone},
        readiness::ReadyToken,
    };
    let (directory, adb_path) = fixture();
    let slot: Arc<Mutex<Option<Weak<Engine>>>> = Arc::new(Mutex::new(None));
    let claim_slot = slot.clone();
    let (played, tones) = mpsc::channel();
    let (release, held) = mpsc::channel();
    let held = Mutex::new(held);
    let first = std::sync::atomic::AtomicBool::new(true);
    let audio = Arc::new(AudioDispatcher::start(
        Arc::new(move |tone| {
            played.send(tone).unwrap();
            if first.swap(false, std::sync::atomic::Ordering::SeqCst) {
                held.lock()
                    .unwrap()
                    .recv_timeout(Duration::from_secs(5))
                    .map_err(|e| e.to_string())?;
            }
            Ok(())
        }),
        Arc::new(move |token| {
            claim_slot
                .lock()
                .unwrap()
                .as_ref()
                .and_then(Weak::upgrade)
                .is_some_and(|engine| engine.claim_ready_cue(token))
        }),
        Arc::new(|_, error| panic!("unexpected audio error: {error}")),
    ));
    let queued = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let queued_output = queued.clone();
    let errors = Arc::new(Mutex::new(Vec::new()));
    let error_output = errors.clone();
    let audio_output = audio.clone();
    let engine = Arc::new(
        Engine::start(
            Settings {
                adb_path,
                ready_delay_ms: 500,
                ..Default::default()
            },
            directory.path().join("observer.apk"),
            Arc::new(move |event, value| {
                if event == "speech-ready" {
                    audio_output.ready(serde_json::from_value::<ReadyToken>(value).unwrap());
                    queued_output.store(true, std::sync::atomic::Ordering::SeqCst);
                } else if event == "operation-error" {
                    error_output.lock().unwrap().push(value);
                }
            }),
        )
        .unwrap(),
    );
    *slot.lock().unwrap() = Some(Arc::downgrade(&engine));
    wait_until(|| engine.snapshot().predicted == RecordingState::Idle);
    audio.feedback(false);
    assert_eq!(
        tones.recv_timeout(Duration::from_secs(2)).unwrap(),
        Tone::Failure
    );
    engine.dispatch(RecordingAction::Start).unwrap();
    wait_until(|| queued.load(std::sync::atomic::Ordering::SeqCst));
    std::fs::write(directory.path().join("hold-observation"), b"").unwrap();
    inject_observer_event(
        directory.path(),
        &serde_json::json!({"type":"error","message":"State observation failed: IllegalStateException"}),
    );
    wait_until(|| {
        errors
            .lock()
            .unwrap()
            .iter()
            .any(|v| v["code"] == "observer_failed")
    });
    // A later success sentinel proves the worker passed the cancelled entry.
    audio.feedback(true);
    release.send(()).unwrap();
    assert_eq!(
        tones.recv_timeout(Duration::from_secs(2)).unwrap(),
        Tone::Success
    );
    engine.shutdown();
    audio.shutdown();
    assert!(tones.try_recv().is_err());
}
