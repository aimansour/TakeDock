mod common;
use common::*;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use takedock_files::Root;
use takedock_lib::{
    discard::DeleteOutcome,
    files::{FileService, JobKind, Video},
};

fn video(fixture: &Fixture, name: &str) -> Video {
    let entry = Root::open(&fixture.path().join("videos"))
        .unwrap()
        .inspect(name)
        .unwrap();
    Video {
        name: entry.name,
        size: entry.identity.size,
        modified_ms: (entry.identity.modified_ns / 1_000_000) as u64,
        ready: entry.ready,
        duration_ms: entry.duration_ms,
        identity: serde_json::to_string(&entry.identity).unwrap(),
    }
}
fn service(fixture: &Fixture) -> FileService {
    FileService::start(fixture.path().into(), Arc::new(|_, _| {}))
}
fn discard(
    files: &FileService,
    fixture: &Fixture,
    allow: Arc<AtomicBool>,
) -> (String, Arc<Mutex<Vec<DeleteOutcome>>>) {
    let outcomes = Arc::new(Mutex::new(Vec::new()));
    let output = outcomes.clone();
    let id = files
        .start_discard_delete(
            fixture.lease.clone(),
            video(fixture, "take.mp4"),
            "op-1".into(),
            Arc::new(move || allow.load(Ordering::Acquire)),
            Arc::new(move |outcome| output.lock().unwrap().push(outcome)),
        )
        .unwrap();
    (id, outcomes)
}
fn outcome(outcomes: &Arc<Mutex<Vec<DeleteOutcome>>>) -> DeleteOutcome {
    wait(|| !outcomes.lock().unwrap().is_empty());
    let outcomes = outcomes.lock().unwrap();
    assert_eq!(outcomes.len(), 1);
    outcomes[0].clone()
}
#[test]
fn cancel_before_worker_authorization_keeps_original() {
    let fixture = Fixture::new();
    std::fs::write(fixture.video("take.mp4"), movie()).unwrap();
    fixture.flag("hold-delete-prepared");
    let files = service(&fixture);
    let allow = Arc::new(AtomicBool::new(true));
    let (_, results) = discard(&files, &fixture, allow.clone());
    wait(|| fixture.path().join("delete-prepared").exists());
    allow.store(false, Ordering::Release);
    fixture.unflag("hold-delete-prepared");
    assert!(matches!(outcome(&results), DeleteOutcome::Retained(_)));
    assert_eq!(std::fs::read(fixture.video("take.mp4")).unwrap(), movie());
    files.shutdown();
}
#[test]
fn successful_handshake_deletes_only_exact_identity() {
    let fixture = Fixture::new();
    std::fs::write(fixture.video("take.mp4"), movie()).unwrap();
    std::fs::write(fixture.video("other.mp4"), b"unrelated").unwrap();
    let files = service(&fixture);
    let (_, results) = discard(&files, &fixture, Arc::new(AtomicBool::new(true)));
    let actual = outcome(&results);
    assert!(matches!(actual, DeleteOutcome::Deleted), "{actual:?}");
    assert!(!fixture.video("take.mp4").exists());
    let calls = std::fs::read_to_string(fixture.path().join("delete-calls.jsonl")).unwrap();
    let calls = calls
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0]["operation"], "op-1");
    assert_eq!(calls[0]["name"], "take.mp4");
    assert!(calls[0]["identity"].as_str().is_some());
    assert_eq!(
        std::fs::read(fixture.video("other.mp4")).unwrap(),
        b"unrelated"
    );
    files.shutdown();
}
#[test]
fn authorized_delete_with_lost_result_is_unknown() {
    let fixture = Fixture::new();
    std::fs::write(fixture.video("take.mp4"), movie()).unwrap();
    fixture.flag("drop-delete-result");
    let files = service(&fixture);
    let (_, results) = discard(&files, &fixture, Arc::new(AtomicBool::new(true)));
    assert!(matches!(outcome(&results), DeleteOutcome::Unknown(_)));
    assert!(!fixture.video("take.mp4").exists());
    files.shutdown();
}
#[test]
fn guarded_delete_is_serialized_with_ordinary_jobs() {
    let fixture = Fixture::new();
    std::fs::write(fixture.video("take.mp4"), movie()).unwrap();
    std::fs::write(fixture.video("old.mp4"), movie()).unwrap();
    fixture.flag("hold-handoff-before-permit");
    let files = service(&fixture);
    files
        .start_job(
            fixture.lease.clone(),
            JobKind::Mark,
            vec![video(&fixture, "old.mp4")],
            None,
            None,
        )
        .unwrap();
    wait(|| fixture.path().join("handoff-started").exists());
    let (_, results) = discard(&files, &fixture, Arc::new(AtomicBool::new(true)));
    assert!(!fixture.path().join("delete-prepared").exists());
    fixture.unflag("hold-handoff-before-permit");
    assert!(matches!(outcome(&results), DeleteOutcome::Deleted));
    assert_eq!(
        std::fs::read(fixture.path().join("handoff/old.mp4")).unwrap(),
        movie()
    );
    files.shutdown();
}
#[test]
fn late_cancellation_cannot_misreport_a_committed_delete() {
    let fixture = Fixture::new();
    std::fs::write(fixture.video("take.mp4"), movie()).unwrap();
    fixture.flag("hold-delete-result");
    let files = service(&fixture);
    let (id, results) = discard(&files, &fixture, Arc::new(AtomicBool::new(true)));
    wait(|| fixture.path().join("delete-result-held").exists());
    assert!(!fixture.video("take.mp4").exists());
    files.cancel(&id).unwrap();
    fixture.unflag("hold-delete-result");
    assert!(matches!(outcome(&results), DeleteOutcome::Deleted));
    files.shutdown();
}

#[test]
fn real_helper_permit_eof_wrong_operation_and_deadline_retain_source() {
    use std::{
        io::{BufRead, BufReader, Write},
        sync::mpsc,
        time::Duration,
    };
    for mode in ["eof", "wrong", "timeout"] {
        let fixture = Fixture::new();
        std::fs::write(fixture.video("take.mp4"), movie()).unwrap();
        let expected = video(&fixture, "take.mp4").identity;
        let command = format!(
            "/data/local/tmp/takedock-files-test delete-guarded take.mp4 {} op-deadline",
            takedock_lib::files::quote(&expected).unwrap()
        );
        let mut child = ChildGuard(
            fixture
                .lease
                .adb
                .spawn(
                    Some(&fixture.lease.serial),
                    &["shell", "-T", &command],
                    true,
                )
                .unwrap(),
        );
        let mut input = child.0.stdin.take();
        let output = child.0.stdout.take().unwrap();
        let (sender, receiver) = mpsc::channel();
        let reader = std::thread::spawn(move || {
            for line in BufReader::new(output).lines() {
                if sender.send(line.unwrap()).is_err() {
                    break;
                }
            }
        });
        let prepared: serde_json::Value =
            serde_json::from_str(&receiver.recv_timeout(Duration::from_secs(5)).unwrap()).unwrap();
        assert_eq!(prepared["type"], "prepared");
        if mode == "eof" {
            drop(input.take());
        }
        if mode == "wrong" {
            input
                .as_mut()
                .unwrap()
                .write_all(b"{\"type\":\"permit\",\"operation_id\":\"other\",\"allow\":true}\n")
                .unwrap();
            input.as_mut().unwrap().flush().unwrap();
        }
        let result: serde_json::Value =
            serde_json::from_str(&receiver.recv_timeout(Duration::from_secs(7)).unwrap()).unwrap();
        assert_eq!(result["status"], "retained");
        if mode == "timeout" {
            assert_eq!(result["code"], "discard_permit_timeout");
        }
        assert_eq!(std::fs::read(fixture.video("take.mp4")).unwrap(), movie());
        drop(input);
        drop(child);
        reader.join().unwrap();
    }
}
