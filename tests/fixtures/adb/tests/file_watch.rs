mod common;
use common::*;
use std::{
    sync::{Arc, Mutex},
    time::Instant,
};
use takedock_files::{Root, watch::WatchBody};
use takedock_lib::{
    file_watch::{FileWatch, WatchUpdate},
    files::quote,
};

fn watcher(fixture: &Fixture) -> (FileWatch, Arc<Mutex<Vec<WatchUpdate>>>) {
    let updates = Arc::new(Mutex::new(Vec::new()));
    let collected = updates.clone();
    let origin = Instant::now();
    let watcher = FileWatch::start(
        fixture.lease.clone(),
        fixture.path().into(),
        Arc::new(move || origin.elapsed().as_millis() as u64),
        Arc::new(move |event| collected.lock().unwrap().push(event)),
    );
    (watcher, updates)
}
fn baseline(updates: &Arc<Mutex<Vec<WatchUpdate>>>) -> bool {
    updates.lock().unwrap().iter().any(|update| matches!(update, WatchUpdate::Frame { evidence: Some(evidence), .. } if evidence.complete))
}

#[test]
fn watch_starts_while_handoff_holds_mutation_lock() {
    let fixture = Fixture::new();
    std::fs::write(fixture.video("old.mp4"), movie()).unwrap();
    let identity = Root::open(&fixture.path().join("videos"))
        .unwrap()
        .stat("old.mp4")
        .unwrap();
    let command = format!(
        "/data/local/tmp/takedock-files-test handoff-guarded old.mp4 {} 4fb0ef42-f12b-4b61-ae88-cf0bd0650084",
        quote(&serde_json::to_string(&identity).unwrap()).unwrap()
    );
    fixture.flag("hold-handoff-before-permit");
    let _handoff = ChildGuard(
        fixture
            .lease
            .adb
            .spawn(
                Some(&fixture.lease.serial),
                &["shell", "-T", &command],
                false,
            )
            .unwrap(),
    );
    wait(|| fixture.path().join("handoff-started").exists());
    let (watch, updates) = watcher(&fixture);
    wait(|| baseline(&updates));
    assert!(fixture.path().join("hold-handoff-before-permit").exists());
    watch.shutdown();
    fixture.unflag("hold-handoff-before-permit");
}

#[test]
fn watch_and_inspect_preserve_unicode_identity() {
    let fixture = Fixture::new();
    std::fs::write(fixture.video("رحلة.mp4"), movie()).unwrap();
    let expected = Root::open(&fixture.path().join("videos"))
        .unwrap()
        .stat("رحلة.mp4")
        .unwrap();
    let (watch, updates) = watcher(&fixture);
    wait(|| baseline(&updates));
    let entries = updates
        .lock()
        .unwrap()
        .iter()
        .find_map(|event| match event {
            WatchUpdate::Frame {
                evidence: Some(evidence),
                ..
            } if evidence.complete => Some(evidence.entries.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(entries["رحلة.mp4"], expected);
    watch.inspect("op-1".into(), "رحلة.mp4".into()).unwrap();
    wait(|| {
        updates.lock().unwrap().iter().any(|update| matches!(update, WatchUpdate::Inspection { operation_id, entry: Ok(entry), .. } if operation_id == "op-1" && entry.identity == expected && entry.ready))
    });
    std::fs::write(fixture.video("new.mp4"), movie()).unwrap();
    wait(|| {
        updates.lock().unwrap().iter().any(|update| matches!(update, WatchUpdate::Frame { frame, evidence: Some(evidence) } if matches!(&frame.body, WatchBody::Created { entry } if entry.name == "new.mp4") && evidence.entries.contains_key("new.mp4")))
    });
    watch.shutdown();
}

#[test]
fn malformed_frame_and_heartbeat_loss_fail_closed() {
    for flag in ["malformed-watch", "stop-watch-heartbeats", "watch-gap"] {
        let fixture = Fixture::new();
        if flag == "malformed-watch" {
            fixture.flag(flag);
        }
        let (watch, updates) = watcher(&fixture);
        if flag != "malformed-watch" {
            wait(|| baseline(&updates));
            fixture.flag(flag);
        }
        wait(|| {
            updates
                .lock()
                .unwrap()
                .iter()
                .any(|update| matches!(update, WatchUpdate::Lost(_)))
        });
        assert_eq!(
            updates
                .lock()
                .unwrap()
                .iter()
                .filter(|update| matches!(update, WatchUpdate::Lost(_)))
                .count(),
            1
        );
        watch.shutdown();
    }
}

#[test]
fn shutdown_cancels_deployment_and_reaps_processes() {
    let fixture = Fixture::new();
    let (watch, updates) = watcher(&fixture);
    wait(|| baseline(&updates));
    let pid = std::fs::read_to_string(fixture.path().join("watch-pid"))
        .unwrap()
        .parse()
        .unwrap();
    assert!(alive(pid));
    watch.shutdown();
    assert!(!alive(pid));
    let other = Fixture::new();
    other.flag("hold-helper-push");
    let (watch, _) = watcher(&other);
    wait(|| other.path().join("helper-push-started").exists());
    let pid = std::fs::read_to_string(other.path().join("helper-push-pid"))
        .unwrap()
        .parse()
        .unwrap();
    watch.shutdown();
    assert!(!alive(pid));
    other.unflag("hold-helper-push");
}
