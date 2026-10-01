use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use takedock_lib::{
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
        std::fs::create_dir(root.path().join("destination")).unwrap();
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
        let sink: takedock_lib::session::EventSink = Arc::new(move |name, value| {
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
                self.root.path().join("destination"),
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
        self.events
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find(|event| event["id"] == id)
            .unwrap()
            .clone()
    }
    fn original(&self, name: &str) -> std::path::PathBuf {
        self.root.path().join("videos").join(name)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.engine.shutdown();
        self.files.shutdown();
    }
}
#[test]
fn batch_copy_publishes_exact_bytes_and_move_deletes_only_after_verification() {
    let fixture = Fixture::new(1000);
    let id = fixture.job(JobKind::Copy, fixture.videos.clone());
    let result = fixture.result(&id);
    assert_eq!(result["status"], "completed", "{result}");
    for video in &fixture.videos {
        assert_eq!(
            std::fs::read(fixture.original(&video.name)).unwrap(),
            std::fs::read(fixture.root.path().join("destination").join(&video.name)).unwrap()
        );
    }
    assert!(!fixture.root.path().join("delete-called").exists());
    let other = Fixture::new(1000);
    let id = other.job(JobKind::Move, other.videos.clone());
    let result = other.result(&id);
    assert_eq!(result["status"], "completed", "{result}");
    for video in &other.videos {
        assert!(!other.original(&video.name).exists());
    }
}
#[test]
fn failed_hash_and_collisions_retain_original_and_existing_destination() {
    let fixture = Fixture::new(1000);
    std::fs::write(fixture.root.path().join("bad-hash"), b"").unwrap();
    let video = fixture.videos[0].clone();
    let id = fixture.job(JobKind::Move, vec![video.clone()]);
    assert_eq!(fixture.result(&id)["status"], "failed");
    assert!(fixture.original(&video.name).exists());
    assert!(!fixture.root.path().join("delete-called").exists());
    assert_eq!(
        std::fs::read_dir(fixture.root.path().join("destination"))
            .unwrap()
            .count(),
        0
    );
    std::fs::remove_file(fixture.root.path().join("bad-hash")).unwrap();
    let target = fixture.root.path().join("destination").join(&video.name);
    std::fs::write(&target, b"keep").unwrap();
    let id = fixture.job(JobKind::Move, vec![video]);
    assert_eq!(fixture.result(&id)["status"], "failed");
    assert_eq!(std::fs::read(target).unwrap(), b"keep");
}
#[test]
fn cancellation_removes_partial_and_never_deletes_original() {
    let fixture = Fixture::new(400_000);
    std::fs::write(fixture.root.path().join("slow-read"), b"").unwrap();
    let video = fixture.videos[0].clone();
    let id = fixture.job(JobKind::Move, vec![video.clone()]);
    wait(|| fixture.root.path().join("read-started").exists());
    fixture.files.cancel(&id).unwrap();
    assert_eq!(fixture.result(&id)["status"], "cancelled");
    assert!(fixture.original(&video.name).exists());
    assert!(!fixture.root.path().join("delete-called").exists());
    assert_eq!(
        std::fs::read_dir(fixture.root.path().join("destination"))
            .unwrap()
            .count(),
        0
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
                Path::new("").into(),
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
                Path::new("").into(),
                Some("new".into())
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
            Path::new("").into(),
            Some("رحلة".into()),
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
            Path::new("").into(),
            Some("renamed".into()),
        )
        .unwrap();
    assert_eq!(fixture.result(&id)["status"], "completed");
    assert!(!fixture.original(&video.name).exists());
    assert!(fixture.original("renamed.mp4").exists());
}
#[test]
fn cancellation_interrupts_hash_verification_while_capture_dispatch_stays_available() {
    use takedock_lib::model::{RecordingAction, RecordingState};
    let fixture = Fixture::new(10);
    wait(|| fixture.engine.snapshot().predicted == RecordingState::Idle);
    std::fs::write(fixture.root.path().join("hold-hash"), b"").unwrap();
    let video = fixture.videos[0].clone();
    let id = fixture.job(JobKind::Move, vec![video.clone()]);
    wait(|| fixture.root.path().join("hash-started").exists());
    fixture.engine.dispatch(RecordingAction::Start).unwrap();
    fixture.engine.dispatch(RecordingAction::Pause).unwrap();
    wait(|| {
        std::fs::read_to_string(fixture.root.path().join("commands.jsonl"))
            .unwrap_or_default()
            .lines()
            .count()
            == 2
    });
    fixture.files.cancel(&id).unwrap();
    let start = Instant::now();
    while !fixture
        .events
        .lock()
        .unwrap()
        .iter()
        .any(|event| event["id"] == id && event["status"] == "cancelled")
        && start.elapsed() < Duration::from_secs(2)
    {
        std::thread::sleep(Duration::from_millis(10));
    }
    let interrupted = fixture
        .events
        .lock()
        .unwrap()
        .iter()
        .any(|event| event["id"] == id && event["status"] == "cancelled");
    std::fs::remove_file(fixture.root.path().join("hold-hash")).unwrap();
    assert!(
        interrupted,
        "hash cancellation must finish without waiting for the phone"
    );
    assert!(fixture.original(&video.name).exists());
    assert!(!fixture.root.path().join("delete-called").exists());
}
#[test]
fn batch_collision_keeps_per_file_results_and_continues_other_entries() {
    let fixture = Fixture::new(10);
    std::fs::write(
        fixture
            .root
            .path()
            .join("destination")
            .join(&fixture.videos[0].name),
        b"keep",
    )
    .unwrap();
    let id = fixture.job(JobKind::Copy, fixture.videos.clone());
    let result = fixture.result(&id);
    assert_eq!(result["status"], "failed");
    assert_eq!(result["completed"], 1);
    assert_eq!(result["results"][0]["status"], "failed");
    assert_eq!(result["results"][1]["status"], "completed");
    assert!(
        fixture
            .root
            .path()
            .join("destination")
            .join(&fixture.videos[1].name)
            .exists()
    );
}
