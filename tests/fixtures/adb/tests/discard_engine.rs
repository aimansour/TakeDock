mod common;
use common::*;
use serde_json::Value;
use std::sync::{Arc, Mutex};
use takedock_lib::{
    discard::DiscardPhase,
    files::{FileService, JobKind},
    model::{RecordingAction, RecordingState, Settings, SpeechReadiness},
    session::Engine,
};

struct Recording {
    fixture: Fixture,
    engine: Engine,
    files: Arc<FileService>,
    events: Arc<Mutex<Vec<(String, Value)>>>,
}
impl Recording {
    fn new(flags: &[&str]) -> Self {
        let fixture = Fixture::new();
        fixture.flag("record-take-files");
        for flag in flags {
            fixture.flag(flag);
        }
        let events = Arc::new(Mutex::new(Vec::new()));
        let collected = events.clone();
        let sink: takedock_lib::session::EventSink =
            Arc::new(move |name, value| collected.lock().unwrap().push((name.into(), value)));
        let files = Arc::new(FileService::start(fixture.path().into(), sink.clone()));
        let engine = Engine::start_with_files(
            Settings {
                adb_path: fixture.lease.adb.path.to_str().unwrap().into(),
                verification_ms: 3000,
                ready_delay_ms: 2000,
                ..Default::default()
            },
            fixture.path().join("observer.apk"),
            fixture.path().into(),
            files.clone(),
            sink,
        )
        .unwrap();
        wait(|| {
            engine.snapshot().observer_ready && engine.snapshot().observed == RecordingState::Idle
        });
        if !flags.contains(&"hold-watch-baseline") {
            wait(|| engine.snapshot().discard.reason == "discard_ready_for_start");
        }
        Self {
            fixture,
            engine,
            files,
            events,
        }
    }
    fn start(&self) -> takedock_lib::discard::TakeToken {
        self.engine.dispatch(RecordingAction::Start).unwrap();
        wait(|| self.engine.snapshot().observed == RecordingState::Recording);
        self.engine.snapshot().discard.take.unwrap()
    }
    fn request(
        &self,
        take: takedock_lib::discard::TakeToken,
    ) -> takedock_lib::discard::DiscardReceipt {
        self.engine
            .request_discard(take.clone(), take.generation)
            .unwrap()
    }
    fn completed(&self) {
        wait(|| self.engine.snapshot().discard.phase == DiscardPhase::Completed);
    }
    fn current_file(&self) -> std::path::PathBuf {
        self.fixture.video(
            std::fs::read_to_string(self.fixture.path().join("current-take-name"))
                .unwrap()
                .trim(),
        )
    }
    fn commands(&self) -> Vec<Value> {
        jsonl::complete_json_lines(
            &std::fs::read(self.fixture.path().join("commands.jsonl")).unwrap_or_default(),
        )
    }
}
impl Drop for Recording {
    fn drop(&mut self) {
        self.fixture.unflag("hold-delete-prepared");
        self.fixture.unflag("hold-hash");
        self.fixture.unflag("hold-delete-result");
        self.fixture.unflag("hold-index");
        self.engine.shutdown();
        self.files.shutdown();
    }
}

#[test]
fn acknowledged_delete_remains_completed_during_held_index_disconnect() {
    let app = Recording::new(&["hold-index"]);
    let take = app.start();
    let path = app.current_file();
    let receipt = app.request(take);
    wait(|| app.fixture.path().join("index-started").exists());
    assert!(!path.exists());
    std::fs::write(app.fixture.path().join("devices.txt"), b"").unwrap();
    wait(|| !app.engine.snapshot().connected);
    let events = app.events.lock().unwrap();
    let results = events
        .iter()
        .filter(|(event, value)| {
            event == "discard-result" && value["operation_id"] == receipt.operation_id
        })
        .map(|(_, value)| value.clone())
        .collect::<Vec<_>>();
    drop(events);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["status"], "completed");
}
#[test]
fn duplicate_requests_during_stop_confirmation_remain_idempotent() {
    let app = Recording::new(&[]);
    let take = app.start();
    app.fixture.flag("hold-observation");
    let first = app.request(take.clone());
    wait(|| app.commands().len() == 2);
    let second = app.request(take);
    assert_eq!(first.operation_id, second.operation_id);
    app.fixture.unflag("hold-observation");
    app.completed();
    assert_eq!(app.commands().len(), 2);
}

#[test]
fn stale_generation_cannot_discard_a_replacement_take_or_cancel_its_cue() {
    let app = Recording::new(&[]);
    let old = app.start();
    std::fs::write(app.fixture.path().join("devices.txt"), b"").unwrap();
    wait(|| !app.engine.snapshot().connected);
    std::fs::write(
        app.fixture.path().join("state.json"),
        br#"{"state":"idle","foreground":true,"video_mode":true}"#,
    )
    .unwrap();
    std::fs::write(
        app.fixture.path().join("devices.txt"),
        b"fixture-one\tdevice\n",
    )
    .unwrap();
    wait(|| {
        app.engine.snapshot().generation != old.generation
            && app.engine.snapshot().discard.reason == "discard_ready_for_start"
    });
    let newer = app.start();
    let readiness = app.engine.snapshot().speech_readiness;
    assert!(
        app.engine
            .request_discard(old.clone(), old.generation)
            .is_err()
    );
    assert_eq!(app.engine.snapshot().discard.take, Some(newer));
    assert!(app.engine.snapshot().discard.eligible);
    assert_eq!(app.engine.snapshot().speech_readiness, readiness);
}

#[test]
fn one_stop_after_confirmation_and_600ms() {
    let app = Recording::new(&["ignore-early-stop"]);
    let take = app.start();
    let path = app.current_file();
    let receipt = app.request(take.clone());
    let again = app.request(take);
    assert_eq!(receipt.operation_id, again.operation_id);
    app.completed();
    assert_eq!(app.commands().len(), 2);
    assert!(!path.exists());
    let commands = app.commands();
    assert!(commands[1]["time"].as_u64().unwrap() - commands[0]["time"].as_u64().unwrap() >= 500);
    assert_eq!(
        app.events
            .lock()
            .unwrap()
            .iter()
            .filter(|(name, event)| name == "discard-result"
                && event["operation_id"] == receipt.operation_id)
            .count(),
        1
    );
}
#[test]
fn discard_cancels_ready_cue_before_waiting_for_start() {
    let app = Recording::new(&[]);
    app.fixture.flag("hold-observation");
    app.engine.dispatch(RecordingAction::Start).unwrap();
    wait(|| app.commands().len() == 1);
    let take = app.engine.snapshot().discard.take.unwrap();
    app.request(take);
    assert_eq!(
        app.engine.snapshot().speech_readiness,
        SpeechReadiness::Inactive
    );
    std::thread::sleep(std::time::Duration::from_millis(750));
    assert_eq!(app.commands().len(), 1);
    app.fixture.unflag("hold-observation");
    app.completed();
    assert!(
        !app.events
            .lock()
            .unwrap()
            .iter()
            .any(|(name, _)| name == "speech-ready")
    );
}
#[test]
fn no_delete_until_own_stop_and_two_stable_inspections() {
    let app = Recording::new(&[]);
    let take = app.start();
    let path = app.current_file();
    app.fixture.flag("hold-observation");
    app.request(take);
    wait(|| app.commands().len() == 2);
    assert!(path.exists());
    assert!(!app.fixture.path().join("delete-prepared").exists());
    app.fixture.unflag("hold-observation");
    app.completed();
    assert!(!path.exists());
}
#[test]
fn ordinary_stop_remains_immediate_while_observation_is_held() {
    let app = Recording::new(&[]);
    app.fixture.flag("hold-observation");
    app.engine.dispatch(RecordingAction::Start).unwrap();
    app.engine.dispatch(RecordingAction::Stop).unwrap();
    wait(|| app.commands().len() == 2);
    assert!(app.fixture.path().join("hold-observation").exists());
    assert!(!app.fixture.path().join("delete-prepared").exists());
}
#[test]
fn paused_take_can_be_discarded() {
    let app = Recording::new(&[]);
    let take = app.start();
    app.engine.dispatch(RecordingAction::Pause).unwrap();
    wait(|| app.engine.snapshot().observed == RecordingState::Paused);
    app.request(take);
    app.completed();
    assert_eq!(app.commands().len(), 3);
}
#[test]
fn missing_baseline_at_start_cannot_be_repaired_later() {
    let app = Recording::new(&["hold-watch-baseline"]);
    let take = app.start();
    assert!(!app.engine.snapshot().discard.eligible);
    assert!(
        app.engine
            .request_discard(take.clone(), take.generation)
            .is_err()
    );
    app.fixture.unflag("hold-watch-baseline");
    app.engine.dispatch(RecordingAction::Pause).unwrap();
    wait(|| app.engine.snapshot().observed == RecordingState::Paused);
    assert!(!app.engine.snapshot().discard.eligible);
    assert!(!app.fixture.path().join("delete-prepared").exists());
}
#[test]
fn new_intent_cancels_deferred_discard_stop() {
    let app = Recording::new(&[]);
    let take = app.start();
    app.request(take);
    app.engine.dispatch(RecordingAction::Pause).unwrap();
    wait(|| app.engine.snapshot().observed == RecordingState::Paused);
    std::thread::sleep(std::time::Duration::from_millis(750));
    assert_eq!(app.commands().len(), 2);
    assert_eq!(app.engine.snapshot().discard.phase, DiscardPhase::Retained);
    assert!(app.current_file().exists());
}
#[test]
fn disconnect_or_observer_error_retains_uncommitted_take() {
    for condition in ["disconnect", "observer-error"] {
        let app = Recording::new(&[]);
        let take = app.start();
        let path = app.current_file();
        app.fixture.flag("hold-observation");
        app.request(take);
        if condition == "disconnect" {
            std::fs::write(app.fixture.path().join("devices.txt"), b"").unwrap();
            wait(|| !app.engine.snapshot().connected);
        } else {
            std::fs::write(
                app.fixture.path().join("inject-event.json"),
                br#"{"type":"error","message":"fixture read failure"}"#,
            )
            .unwrap();
            wait(|| app.engine.snapshot().discard.phase == DiscardPhase::Retained);
        }
        assert!(path.exists());
        assert!(!app.fixture.path().join("delete-prepared").exists());
    }
}
#[test]
fn new_start_during_held_delete_authorization_retains_old_file() {
    let app = Recording::new(&["hold-delete-prepared"]);
    let take = app.start();
    let original = app.current_file();
    app.request(take);
    wait(|| app.fixture.path().join("delete-prepared").exists());
    app.engine.dispatch(RecordingAction::Start).unwrap();
    wait(|| app.engine.snapshot().observed == RecordingState::Recording);
    let newer = app.current_file();
    let bytes = std::fs::read(&newer).unwrap();
    app.fixture.unflag("hold-delete-prepared");
    wait(|| {
        app.events
            .lock()
            .unwrap()
            .iter()
            .any(|(name, _)| name == "discard-result")
    });
    assert!(original.exists());
    assert_eq!(std::fs::read(newer).unwrap(), bytes);
}
#[test]
fn finalization_waits_without_retrying_the_stop() {
    let app = Recording::new(&["hold-finalization"]);
    let take = app.start();
    app.request(take);
    wait(|| app.engine.snapshot().discard.phase == DiscardPhase::Finalizing);
    assert!(!app.fixture.path().join("delete-prepared").exists());
    app.fixture.unflag("hold-finalization");
    app.completed();
    assert_eq!(app.commands().len(), 2);
}
#[test]
fn held_copy_does_not_delay_ordinary_capture_or_revive_cancelled_delete() {
    let app = Recording::new(&[]);
    std::fs::write(app.fixture.video("old.mp4"), movie()).unwrap();
    let videos = app.files.list(app.engine.lease().unwrap()).unwrap();
    let old = videos
        .into_iter()
        .find(|video| video.name == "old.mp4")
        .unwrap();
    app.fixture.flag("hold-hash");
    app.files
        .start_job(
            app.engine.lease().unwrap(),
            JobKind::Copy,
            vec![old],
            app.fixture.path().join("destination"),
            None,
        )
        .unwrap();
    wait(|| app.fixture.path().join("hash-started").exists());
    // The background watcher must establish a fresh complete idle manifest after this unrelated write.
    wait(|| app.engine.snapshot().discard.reason == "discard_ready_for_start");
    let take = app.start();
    let path = app.current_file();
    app.request(take);
    wait(|| app.engine.snapshot().observed == RecordingState::Idle);
    wait(|| app.engine.snapshot().discard.phase == DiscardPhase::Deleting);
    app.engine.dispatch(RecordingAction::Start).unwrap();
    wait(|| app.commands().len() == 3);
    app.fixture.unflag("hold-hash");
    wait(|| app.fixture.path().join("destination/old.mp4").exists());
    wait(|| {
        app.events.lock().unwrap().iter().any(|(event, value)| {
            event == "file-job"
                && value["discard_operation_id"].is_string()
                && value["status"] == "cancelled"
        })
    });
    assert!(!app.fixture.path().join("delete-calls.jsonl").exists());
    assert!(path.exists());
}
