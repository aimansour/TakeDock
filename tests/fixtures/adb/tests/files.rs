use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use takedock_lib::{
    file_results::FileResultStore,
    files::{FileService, JobKind, Video},
    model::Settings,
    session::Engine,
};
fn wait(mut predicate: impl FnMut() -> bool) {
    let start = Instant::now();
    while !predicate() {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "condition not reached"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn mp4(payload: usize) -> Vec<u8> {
    fn atom(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut bytes = ((payload.len() + 8) as u32).to_be_bytes().to_vec();
        bytes.extend(kind);
        bytes.extend(payload);
        bytes
    }
    let mut duration = [0; 20];
    duration[12..16].copy_from_slice(&1000u32.to_be_bytes());
    duration[16..20].copy_from_slice(&100u32.to_be_bytes());
    [
        atom(b"ftyp", b"isom0000"),
        atom(b"mdat", &vec![123; payload]),
        atom(b"moov", &atom(b"mvhd", &duration)),
    ]
    .concat()
}
struct Fixture {
    root: tempfile::TempDir,
    engine: Engine,
    files: FileService,
    events: Arc<Mutex<Vec<serde_json::Value>>>,
    named_events: Arc<Mutex<Vec<(String, serde_json::Value)>>>,
    results: Arc<FileResultStore>,
    videos: Vec<Video>,
}
impl Fixture {
    fn new(size: usize) -> Self {
        let root = tempfile::tempdir().unwrap();
        let adb = root
            .path()
            .join(if cfg!(windows) { "adb.exe" } else { "adb" });
        std::fs::copy(env!("CARGO_BIN_EXE_takedock-adb-fixture"), &adb).unwrap();
        std::fs::write(root.path().join("observer.apk"), b"fixture-apk").unwrap();
        std::fs::create_dir(root.path().join("videos")).unwrap();
        std::fs::create_dir(root.path().join("handoff")).unwrap();
        std::fs::write(root.path().join("handoff/unrelated.mp4"), b"hands off").unwrap();
        std::fs::create_dir_all(root.path().join("file-helper/arm64-v8a")).unwrap();
        std::fs::write(
            root.path().join("file-helper/arm64-v8a/takedock-files"),
            b"fixture-helper",
        )
        .unwrap();
        for name in ["a'b.mp4", "رحلة.mp4"] {
            std::fs::write(root.path().join("videos").join(name), mp4(size)).unwrap();
        }
        let events = Arc::new(Mutex::new(vec![]));
        let output = events.clone();
        let named_events = Arc::new(Mutex::new(vec![]));
        let named_output = named_events.clone();
        let started = Instant::now();
        let results = Arc::new(FileResultStore::new(Arc::new(move || {
            u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
        })));
        let result_output = results.clone();
        let sink: takedock_lib::session::EventSink = Arc::new(move |name, value| {
            result_output.receive(name, &value);
            named_output
                .lock()
                .unwrap()
                .push((name.into(), value.clone()));
            if name == "file-job" {
                output.lock().unwrap().push(value);
            }
        });
        let engine = Engine::start(
            Settings {
                adb_path: adb.to_str().unwrap().into(),
                ..Default::default()
            },
            root.path().join("observer.apk"),
            sink.clone(),
        )
        .unwrap();
        wait(|| engine.snapshot().connected);
        let files = FileService::start(root.path().into(), sink);
        let videos = files.list(engine.lease().unwrap()).unwrap();
        assert_eq!(videos.len(), 2);
        Self {
            root,
            engine,
            files,
            events,
            named_events,
            results,
            videos,
        }
    }
    fn job(&self, kind: JobKind, videos: Vec<Video>) -> String {
        self.files
            .start_job(
                self.engine
                    .lease_for(self.engine.snapshot().generation)
                    .unwrap(),
                kind,
                videos,
                None,
                None,
            )
            .unwrap()
    }
    fn result(&self, id: &str) -> serde_json::Value {
        wait(|| {
            self.events.lock().unwrap().iter().any(|event| {
                event["id"] == id
                    && ["completed", "failed", "cancelled"]
                        .contains(&event["status"].as_str().unwrap())
            })
        });
        let result = self
            .events
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find(|event| event["id"] == id)
            .unwrap()
            .clone();
        eprintln!("terminal: {result}");
        eprintln!(
            "peer outcomes: {}",
            std::fs::read_to_string(self.root.path().join("handoff-outcomes.jsonl"))
                .unwrap_or_default()
        );
        result
    }
    fn original(&self, name: &str) -> std::path::PathBuf {
        self.root.path().join("videos").join(name)
    }
    fn rename(&self, video: Video) -> String {
        self.files
            .start_job(
                self.engine.lease().unwrap(),
                JobKind::Rename,
                vec![video],
                Some("renamed".into()),
                None,
            )
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        for flag in [
            "hold-index",
            "hold-handoff-before-permit",
            "hold-handoff-after-commit",
            "hold-rename-ack",
        ] {
            let _ = std::fs::remove_file(self.root.path().join(flag));
        }
        self.engine.shutdown();
        self.files.shutdown();
    }
}

#[test]
fn batch_mark_relocates_exact_bytes_with_unique_receipts_and_no_payload_requests() {
    let fixture = Fixture::new(20);
    let expected = fixture
        .videos
        .iter()
        .map(|v| {
            (
                v.name.clone(),
                std::fs::read(fixture.original(&v.name)).unwrap(),
            )
        })
        .collect::<Vec<_>>();
    let id = fixture.job(JobKind::Mark, fixture.videos.clone());
    let result = fixture.result(&id);
    assert_eq!(result["status"], "completed");
    assert_eq!(result["completed"], 2);
    for (name, bytes) in expected {
        assert!(!fixture.original(&name).exists());
        assert_eq!(
            std::fs::read(fixture.root.path().join("handoff").join(name)).unwrap(),
            bytes
        );
    }
    let events = fixture.named_events.lock().unwrap();
    let receipts = events
        .iter()
        .filter(|(name, value)| name == "file-handed-off" && value["job_id"] == id)
        .map(|(_, value)| value)
        .collect::<Vec<_>>();
    assert_eq!(receipts.len(), 2);
    assert_ne!(receipts[0]["operation_id"], receipts[1]["operation_id"]);
    assert!(receipts[0]["sequence"].as_u64().unwrap() < receipts[1]["sequence"].as_u64().unwrap());
    drop(events);
    let calls = std::fs::read_to_string(fixture.root.path().join("helper-calls.jsonl")).unwrap();
    for line in calls.lines() {
        let args: Vec<String> = serde_json::from_str(line).unwrap();
        assert!(
            !["read", "hash"].contains(&args[0].as_str()),
            "payload request: {line}"
        );
    }
    fixture.files.shutdown();
    let restarted = FileService::start(fixture.root.path().into(), Arc::new(|_, _| {}));
    assert!(
        restarted
            .list(fixture.engine.lease().unwrap())
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        std::fs::read(fixture.root.path().join("handoff/unrelated.mp4")).unwrap(),
        b"hands off"
    );
    restarted.shutdown();
}

#[test]
fn batch_collision_retains_original_and_continues_other_entries() {
    let fixture = Fixture::new(30);
    let source = &fixture.videos[0];
    let expected = std::fs::read(fixture.original(&source.name)).unwrap();
    let target = fixture.root.path().join("handoff").join(&source.name);
    std::fs::write(&target, b"already here").unwrap();
    let id = fixture.job(JobKind::Mark, fixture.videos.clone());
    let result = fixture.result(&id);
    assert_eq!(result["status"], "failed");
    assert_eq!(result["completed"], 1);
    assert_eq!(result["results"][0]["status"], "retained");
    assert_eq!(result["results"][0]["message"], "destination_exists");
    assert_eq!(std::fs::read(target).unwrap(), b"already here");
    assert_eq!(
        std::fs::read(fixture.original(&source.name)).unwrap(),
        expected
    );
    assert!(!fixture.original(&fixture.videos[1].name).exists());
}

#[test]
fn conflict_created_after_preparation_is_never_overwritten() {
    let fixture = Fixture::new(20);
    let source = fixture.videos[0].clone();
    std::fs::write(fixture.root.path().join("hold-handoff-before-permit"), b"").unwrap();
    let id = fixture.job(JobKind::Mark, vec![source.clone()]);
    wait(|| fixture.root.path().join("handoff-started").exists());
    let target = fixture.root.path().join("handoff").join(&source.name);
    std::fs::write(&target, b"last moment").unwrap();
    std::fs::remove_file(fixture.root.path().join("hold-handoff-before-permit")).unwrap();
    assert_eq!(
        fixture.result(&id)["results"][0]["message"],
        "destination_exists"
    );
    assert!(fixture.original(&source.name).exists());
    assert_eq!(std::fs::read(target).unwrap(), b"last moment");
}

#[test]
fn pending_mark_reserves_the_object_against_mark_delete_rename_and_discard() {
    let fixture = Fixture::new(20);
    let source = fixture.videos[0].clone();
    std::fs::write(fixture.root.path().join("hold-handoff-before-permit"), b"").unwrap();
    let id = fixture.job(JobKind::Mark, vec![source.clone()]);
    wait(|| fixture.root.path().join("handoff-started").exists());
    for kind in [JobKind::Mark, JobKind::Delete, JobKind::Rename] {
        let result = fixture.files.start_job(
            fixture.engine.lease().unwrap(),
            kind,
            vec![source.clone()],
            Some("new".into()),
            None,
        );
        assert_eq!(result.unwrap_err(), "file_busy");
    }
    let discard = fixture.files.start_discard_delete(
        fixture.engine.lease().unwrap(),
        source.clone(),
        "discard".into(),
        Arc::new(|| true),
        Arc::new(|_| {}),
    );
    assert_eq!(discard.unwrap_err(), "file_busy");
    fixture.files.cancel(&id).unwrap();
    assert_eq!(fixture.result(&id)["status"], "cancelled");
    assert!(fixture.original(&source.name).exists());
    std::fs::remove_file(fixture.root.path().join("hold-handoff-before-permit")).unwrap();
    let fresh = fixture
        .files
        .list(fixture.engine.lease().unwrap())
        .unwrap()
        .into_iter()
        .find(|v| v.name == source.name)
        .unwrap();
    let next = fixture.job(JobKind::Delete, vec![fresh]);
    assert_eq!(fixture.result(&next)["status"], "completed");
}

#[test]
fn dropped_commit_ack_is_unknown_once_and_later_listing_does_not_manage_destination() {
    let fixture = Fixture::new(20);
    let source = fixture.videos[0].clone();
    std::fs::write(fixture.root.path().join("drop-handoff-result"), b"").unwrap();
    let id = fixture.job(JobKind::Mark, vec![source.clone()]);
    let result = fixture.result(&id);
    assert_eq!(result["status"], "failed");
    assert_eq!(result["results"][0]["status"], "outcome_unknown");
    assert!(
        fixture
            .named_events
            .lock()
            .unwrap()
            .iter()
            .all(|(name, _)| name != "file-handed-off")
    );
    assert!(!fixture.original(&source.name).exists());
    assert!(
        fixture
            .root
            .path()
            .join("handoff")
            .join(&source.name)
            .exists()
    );
    assert!(
        fixture
            .files
            .list(fixture.engine.lease().unwrap())
            .unwrap()
            .iter()
            .all(|v| v.name != source.name)
    );
    let calls = std::fs::read_to_string(fixture.root.path().join("helper-calls.jsonl")).unwrap();
    assert_eq!(
        calls
            .lines()
            .filter(|line| line.contains("handoff-guarded"))
            .count(),
        1
    );
}

#[test]
fn cancellation_after_permit_waits_for_commit_ack_while_capture_remains_independent() {
    use takedock_lib::model::RecordingAction;
    let fixture = Fixture::new(20);
    let source = fixture.videos[0].clone();
    std::fs::write(fixture.root.path().join("hold-handoff-after-commit"), b"").unwrap();
    let id = fixture.job(JobKind::Mark, vec![source.clone()]);
    wait(|| fixture.root.path().join("handoff-committed").exists());
    fixture.files.cancel(&id).unwrap();
    fixture.engine.dispatch(RecordingAction::Start).unwrap();
    fixture.engine.dispatch(RecordingAction::Stop).unwrap();
    wait(|| {
        std::fs::read_to_string(fixture.root.path().join("commands.jsonl"))
            .unwrap_or_default()
            .lines()
            .count()
            == 2
    });
    assert!(!fixture.original(&source.name).exists());
    std::fs::remove_file(fixture.root.path().join("hold-handoff-after-commit")).unwrap();
    assert_eq!(fixture.result(&id)["status"], "completed");
}

#[test]
fn disconnection_after_permit_is_unknown_without_a_second_mutation() {
    let fixture = Fixture::new(20);
    std::fs::write(fixture.root.path().join("hold-handoff-after-commit"), b"").unwrap();
    let id = fixture.job(JobKind::Mark, vec![fixture.videos[0].clone()]);
    wait(|| fixture.root.path().join("handoff-committed").exists());
    fixture.engine.shutdown();
    assert_eq!(
        fixture.result(&id)["results"][0]["status"],
        "outcome_unknown"
    );
}

#[test]
fn failed_index_is_a_persistent_warning_after_confirmed_handoff() {
    let fixture = Fixture::new(20);
    std::fs::write(fixture.root.path().join("fail-handoff-index"), b"").unwrap();
    let id = fixture.job(JobKind::Mark, vec![fixture.videos[0].clone()]);
    assert_eq!(fixture.result(&id)["status"], "completed");
    wait(|| {
        fixture
            .named_events
            .lock()
            .unwrap()
            .iter()
            .any(|(name, value)| name == "file-index-warning" && value["id"] == id)
    });
    let snapshot = fixture.results.snapshot();
    let notice = snapshot
        .notices
        .iter()
        .find(|notice| notice.key.operation_id == id)
        .unwrap();
    assert!(notice.expires_at_ms.is_none());
    assert!(
        notice
            .warnings
            .iter()
            .any(|warning| warning == "media_index_pending")
    );
}

#[test]
fn rename_update_precedes_held_index_and_next_job() {
    use takedock_lib::model::{RecordingAction, RecordingState};
    let fixture = Fixture::new(20);
    let original = fixture.videos[0].clone();
    std::fs::write(fixture.root.path().join("hold-index"), b"").unwrap();
    std::fs::write(fixture.root.path().join("hold-handoff-before-permit"), b"").unwrap();
    let id = fixture.rename(original.clone());
    wait(|| fixture.root.path().join("index-started").exists());
    let update = fixture
        .named_events
        .lock()
        .unwrap()
        .iter()
        .find(|(event, value)| event == "file-renamed" && value["id"] == id)
        .map(|(_, value)| value.clone());
    assert!(
        update.is_some(),
        "verified rename update must precede media indexing"
    );
    let update = update.unwrap();
    assert!(update["sequence"].as_u64().unwrap() > 0);
    assert_eq!(update["source_name"], original.name);
    assert_eq!(update["source_identity"], original.identity);
    assert_eq!(update["video"]["name"], "renamed.mp4");
    let root = takedock_files::Root::open(&fixture.root.path().join("videos")).unwrap();
    let fresh = root.inspect("renamed.mp4").unwrap();
    assert_eq!(
        update["video"]["identity"],
        serde_json::to_string(&fresh.identity).unwrap()
    );
    assert_eq!(update["video"]["duration_ms"], 100);
    let next = fixture.job(JobKind::Mark, vec![fixture.videos[1].clone()]);
    wait(|| fixture.root.path().join("handoff-started").exists());
    std::fs::remove_file(fixture.root.path().join("hold-index")).unwrap();
    wait(|| fixture.root.path().join("handoff-started").exists());
    wait(|| fixture.engine.snapshot().predicted == RecordingState::Idle);
    fixture.engine.dispatch(RecordingAction::Start).unwrap();
    fixture.engine.dispatch(RecordingAction::Stop).unwrap();
    wait(|| {
        std::fs::read_to_string(fixture.root.path().join("commands.jsonl"))
            .unwrap_or_default()
            .lines()
            .count()
            == 2
    });
    let events = fixture.named_events.lock().unwrap();
    let rename_position = events
        .iter()
        .position(|(name, value)| name == "file-renamed" && value["id"] == id)
        .unwrap();
    let next_position = events
        .iter()
        .position(|(name, value)| {
            name == "file-job" && value["id"] == next && value["status"] == "running"
        })
        .unwrap();
    assert!(rename_position < next_position);
    drop(events);
    std::fs::remove_file(fixture.root.path().join("hold-handoff-before-permit")).unwrap();
    assert_eq!(fixture.result(&id)["status"], "completed");
    assert_eq!(fixture.result(&next)["status"], "completed");
}

#[test]
fn committed_rename_inspection_failure_is_warning() {
    let fixture = Fixture::new(20);
    let source = fixture.videos[0].clone();
    let bytes = std::fs::read(fixture.original(&source.name)).unwrap();
    std::fs::write(fixture.root.path().join("fail-rename-inspect"), b"").unwrap();
    let id = fixture.rename(source);
    let result = fixture.result(&id);
    assert_eq!(result["status"], "completed");
    assert_eq!(result["results"][0]["destination_name"], "renamed.mp4");
    assert_eq!(
        std::fs::read(fixture.original("renamed.mp4")).unwrap(),
        bytes
    );
    let events = fixture.named_events.lock().unwrap();
    assert!(
        events
            .iter()
            .any(|(name, value)| name == "file-metadata-warning"
                && value["id"] == id
                && value["code"] == "rename_refresh_pending")
    );
    assert!(
        !events
            .iter()
            .any(|(name, value)| name == "file-renamed" && value["id"] == id)
    );
}

#[test]
fn late_cancel_after_rename_ack_keeps_completed() {
    for disconnect in [false, true] {
        let fixture = Fixture::new(20);
        std::fs::write(fixture.root.path().join("hold-index"), b"").unwrap();
        let id = fixture.rename(fixture.videos[0].clone());
        wait(|| fixture.root.path().join("index-started").exists());
        if disconnect {
            fixture.engine.shutdown();
        } else {
            if let Err(code) = fixture.files.cancel(&id) {
                assert_eq!(code, "job_not_found");
            }
        }
        std::fs::remove_file(fixture.root.path().join("hold-index")).unwrap();
        let result = fixture.result(&id);
        assert_eq!(result["status"], "completed");
        assert_eq!(result["results"][0]["destination_name"], "renamed.mp4");
        assert!(fixture.original("renamed.mp4").exists());
        let snapshot = fixture.results.snapshot();
        let notice = snapshot
            .history
            .iter()
            .find(|notice| notice.key.operation_id == id)
            .expect("Committed rename must remain in shared history after disconnect");
        assert_eq!(
            notice.results[0].destination_name.as_deref(),
            Some("renamed.mp4")
        );
        assert_eq!(notice.completed, 1);
        assert_eq!(
            notice.key.generation,
            result["generation"].as_u64().unwrap()
        );
    }
}

#[test]
fn unacknowledged_rename_never_patches_rows() {
    let fixture = Fixture::new(20);
    std::fs::write(fixture.root.path().join("hold-rename-ack"), b"").unwrap();
    let id = fixture.rename(fixture.videos[0].clone());
    wait(|| fixture.root.path().join("rename-acknowledged").exists());
    fixture.files.cancel(&id).unwrap();
    let result = fixture.result(&id);
    assert_eq!(result["status"], "cancelled");
    assert!(result["results"][0]["destination_name"].is_null());
    assert!(fixture.original("renamed.mp4").exists()); // mutation may commit before lost ack
    assert!(
        !fixture
            .named_events
            .lock()
            .unwrap()
            .iter()
            .any(|(name, value)| name == "file-renamed" && value["id"] == id)
    );
}
#[test]
fn replacements_and_unfinished_videos_cannot_be_deleted() {
    let fixture = Fixture::new(10);
    let video = fixture.videos[0].clone();
    std::fs::write(fixture.original(&video.name), mp4(100)).unwrap();
    let id = fixture.job(JobKind::Delete, vec![video.clone()]);
    assert_eq!(fixture.result(&id)["status"], "failed");
    assert!(fixture.original(&video.name).exists());
    std::fs::write(fixture.original(&video.name), b"still recording").unwrap();
    let videos = fixture.files.list(fixture.engine.lease().unwrap()).unwrap();
    let unfinished = videos.into_iter().find(|v| v.name == video.name).unwrap();
    assert!(!unfinished.ready);
    assert!(
        fixture
            .files
            .start_job(
                fixture.engine.lease().unwrap(),
                JobKind::Delete,
                vec![unfinished],
                None,
                None
            )
            .is_err()
    );
}
#[test]
fn rename_requires_one_selection_and_never_overwrites() {
    let fixture = Fixture::new(20);
    assert!(
        fixture
            .files
            .start_job(
                fixture.engine.lease().unwrap(),
                JobKind::Rename,
                fixture.videos.clone(),
                Some("new".into()),
                None
            )
            .is_err()
    );
    let video = fixture
        .videos
        .iter()
        .find(|v| v.name == "a'b.mp4")
        .unwrap()
        .clone();
    let id = fixture
        .files
        .start_job(
            fixture.engine.lease().unwrap(),
            JobKind::Rename,
            vec![video.clone()],
            Some("رحلة".into()),
            None,
        )
        .unwrap();
    assert_eq!(fixture.result(&id)["status"], "failed");
    assert!(fixture.original(&video.name).exists());
    // Claim/restore changes Unix ctime even after a collision. The UI refreshes
    // at job completion, so a retry must use its freshly listed identity.
    let video = fixture
        .files
        .list(fixture.engine.lease().unwrap())
        .unwrap()
        .into_iter()
        .find(|item| item.name == video.name)
        .unwrap();
    let id = fixture
        .files
        .start_job(
            fixture.engine.lease().unwrap(),
            JobKind::Rename,
            vec![video.clone()],
            Some("renamed".into()),
            None,
        )
        .unwrap();
    assert_eq!(fixture.result(&id)["status"], "completed");
    assert!(!fixture.original(&video.name).exists());
    assert!(fixture.original("renamed.mp4").exists());
}
#[test]
fn legacy_exec_success_is_not_remote_rename_acknowledgement() {
    let fixture = Fixture::new(20);
    let video = fixture
        .videos
        .iter()
        .find(|video| video.name == "a'b.mp4")
        .unwrap()
        .clone();
    let source_bytes = std::fs::read(fixture.original(&video.name)).unwrap();
    let destination_bytes = std::fs::read(fixture.original("رحلة.mp4")).unwrap();
    let id = fixture
        .files
        .start_job(
            fixture.engine.lease().unwrap(),
            JobKind::Rename,
            vec![video.clone()],
            Some("رحلة".into()),
            None,
        )
        .unwrap();
    let result = fixture.result(&id);
    assert_eq!(
        result["status"], "failed",
        "local exec-out exit 0 cannot acknowledge a remote failure"
    );
    assert!(result["results"][0]["destination_name"].is_null());
    assert_eq!(result["results"][0]["requested_name"], "رحلة.mp4");
    assert_eq!(
        std::fs::read(fixture.original(&video.name)).unwrap(),
        source_bytes
    );
    assert_eq!(
        std::fs::read(fixture.original("رحلة.mp4")).unwrap(),
        destination_bytes
    );
    assert!(
        !fixture
            .named_events
            .lock()
            .unwrap()
            .iter()
            .any(|(name, value)| name == "file-renamed" && value["id"] == id)
    );
}
