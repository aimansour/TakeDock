use std::{cell::Cell, fs};
use takedock_files::{
    Identity, Root,
    handoff::{self, Message, Outcome},
};

const OP: &str = "4fb0ef42-f12b-4b61-ae88-cf0bd0650084";
fn movie() -> Vec<u8> {
    fn atom(kind: &[u8; 4], bytes: &[u8]) -> Vec<u8> {
        let mut data = ((bytes.len() + 8) as u32).to_be_bytes().to_vec();
        data.extend(kind);
        data.extend(bytes);
        data
    }
    let mut timeline = [0; 20];
    timeline[12..16].copy_from_slice(&1000u32.to_be_bytes());
    timeline[16..20].copy_from_slice(&100u32.to_be_bytes());
    [
        atom(b"ftyp", b"isom0000"),
        atom(b"mdat", b"take"),
        atom(b"moov", &atom(b"mvhd", &timeline)),
    ]
    .concat()
}
fn two_roots(name: &str) -> (tempfile::TempDir, Root, Root, Identity) {
    let trial = tempfile::tempdir().unwrap();
    fs::create_dir(trial.path().join("OpenCamera")).unwrap();
    fs::create_dir(trial.path().join("TakeDock")).unwrap();
    fs::write(trial.path().join("OpenCamera").join(name), movie()).unwrap();
    let source = Root::open(&trial.path().join("OpenCamera")).unwrap();
    let target = Root::open(&trial.path().join("TakeDock")).unwrap();
    let identity = source.stat(name).unwrap();
    (trial, source, target, identity)
}

#[test]
fn confirmed_handoff_preserves_object_and_bytes_and_leaves_only_source_management() {
    let name = "رحلة '1.mp4";
    let (trial, source, target, identity) = two_roots(name);
    fs::write(
        trial.path().join("TakeDock/unrelated.mp4"),
        b"outside app ownership",
    )
    .unwrap();
    let outcome = source.handoff_to(&target, name, &identity, OP, || Ok(true));
    let Outcome::HandedOff { receipt, warning } = outcome else {
        panic!("{outcome:?}")
    };
    assert_eq!(receipt.operation_id, OP);
    assert_eq!(receipt.source_name, name);
    assert_eq!(receipt.destination_name, name);
    assert!(warning.is_none());
    assert!(source.list().unwrap().is_empty());
    assert_eq!(target.stat(name).unwrap().inode, identity.inode);
    assert_eq!(target.stat(name).unwrap().size, identity.size);
    assert_eq!(
        fs::read(trial.path().join("TakeDock").join(name)).unwrap(),
        movie()
    );
    source.recover().unwrap();
    assert!(source.list().unwrap().is_empty());
    assert_eq!(
        fs::read(trial.path().join("TakeDock/unrelated.mp4")).unwrap(),
        b"outside app ownership"
    );
}

#[test]
fn collision_at_publish_retains_both() {
    let (trial, source, target, identity) = two_roots("take.mp4");
    let outcome = source.handoff_to(&target, "take.mp4", &identity, OP, || {
        fs::write(
            trial.path().join("TakeDock/take.mp4"),
            b"last moment user file",
        )
        .unwrap();
        Ok(true)
    });
    assert!(matches!(outcome, Outcome::Retained { .. }), "{outcome:?}");
    assert_eq!(
        fs::read(trial.path().join("OpenCamera/take.mp4")).unwrap(),
        movie()
    );
    assert_eq!(
        fs::read(trial.path().join("TakeDock/take.mp4")).unwrap(),
        b"last moment user file"
    );
    source.recover().unwrap();
    assert_eq!(source.list().unwrap().len(), 1);
}

#[test]
fn existing_collision_does_not_even_request_permission() {
    let (trial, source, target, identity) = two_roots("take.mp4");
    fs::write(trial.path().join("TakeDock/take.mp4"), b"existing").unwrap();
    let called = Cell::new(false);
    let outcome = source.handoff_to(&target, "take.mp4", &identity, OP, || {
        called.set(true);
        Ok(true)
    });
    assert!(matches!(outcome, Outcome::Retained { .. }));
    assert!(!called.get());
    assert_eq!(source.stat("take.mp4").unwrap(), identity);
}

#[test]
fn wrong_or_denied_permit_never_publishes() {
    for permit in [
        Ok(Vec::new()),
        Ok(br#"{"type":"permit","operation_id":"wrong","allow":true}"#.to_vec()),
        Err("handoff_permit_timeout".into()),
    ] {
        let (_trial, source, target, identity) = two_roots("take.mp4");
        let mut output = Vec::new();
        handoff::serve(
            &source,
            &target,
            "take.mp4",
            &identity,
            OP,
            || permit,
            &mut output,
        )
        .unwrap();
        let messages: Vec<Message> = output
            .split(|b| *b == b'\n')
            .filter(|s| !s.is_empty())
            .map(|s| serde_json::from_slice(s).unwrap())
            .collect();
        assert!(matches!(&messages[0], Message::Prepared { operation_id } if operation_id == OP));
        assert!(matches!(
            &messages[1],
            Message::Result {
                outcome: Outcome::Retained { .. },
                ..
            }
        ));
        assert_eq!(source.stat("take.mp4").unwrap(), identity);
        assert!(target.list().unwrap().is_empty());
    }
    let (_trial, source, target, identity) = two_roots("take.mp4");
    assert!(matches!(
        source.handoff_to(&target, "take.mp4", &identity, OP, || Ok(false)),
        Outcome::Retained { .. }
    ));
    assert_eq!(source.stat("take.mp4").unwrap(), identity);
}

#[test]
fn changed_or_unfinished_source_is_protected() {
    let (trial, source, target, identity) = two_roots("take.mp4");
    fs::write(
        trial.path().join("OpenCamera/take.mp4"),
        b"unfinished replacement",
    )
    .unwrap();
    let current = source.stat("take.mp4").unwrap();
    for expected in [&identity, &current] {
        let outcome = source.handoff_to(&target, "take.mp4", expected, OP, || {
            panic!("protected source requested permit")
        });
        assert!(matches!(outcome, Outcome::Retained { .. }));
    }
    assert_eq!(
        fs::read(trial.path().join("OpenCamera/take.mp4")).unwrap(),
        b"unfinished replacement"
    );
    assert!(target.list().unwrap().is_empty());
}

#[test]
fn replacement_during_permission_does_not_adopt_the_new_source() {
    let (trial, source, target, identity) = two_roots("take.mp4");
    let outcome = source.handoff_to(&target, "take.mp4", &identity, OP, || {
        fs::rename(
            trial.path().join("OpenCamera/take.mp4"),
            trial.path().join("OpenCamera/original.mp4"),
        )
        .unwrap();
        fs::write(trial.path().join("OpenCamera/take.mp4"), b"new user file").unwrap();
        Ok(true)
    });
    assert!(matches!(outcome, Outcome::Retained { .. }));
    assert_eq!(
        fs::read(trial.path().join("OpenCamera/original.mp4")).unwrap(),
        movie()
    );
    assert_eq!(
        fs::read(trial.path().join("OpenCamera/take.mp4")).unwrap(),
        b"new user file"
    );
    assert!(target.list().unwrap().is_empty());
}

#[test]
fn replacement_destination_directory_is_not_entered() {
    let (trial, source, target, identity) = two_roots("take.mp4");
    let outcome = source.handoff_to(&target, "take.mp4", &identity, OP, || {
        fs::rename(
            trial.path().join("TakeDock"),
            trial.path().join("old-target"),
        )
        .unwrap();
        fs::create_dir(trial.path().join("TakeDock")).unwrap();
        Ok(true)
    });
    assert!(!matches!(outcome, Outcome::HandedOff { .. }));
    assert!(!trial.path().join("TakeDock/take.mp4").exists());
    assert_eq!(
        fs::read(trial.path().join("OpenCamera/take.mp4")).unwrap(),
        movie()
    );
}

fn interrupted_intent(trial: &std::path::Path, identity: &Identity) {
    let journal = trial.join("OpenCamera/.takedock-journal");
    fs::create_dir(&journal).unwrap();
    fs::write(journal.join(format!("{OP}.json")), serde_json::to_vec(&serde_json::json!({
        "version":1,"name":"take.mp4","claim":format!(".takedock-delete-{OP}"),"identity":identity,
        "handoff":{"operation_id":OP,"destination_name":"take.mp4"}
    })).unwrap()).unwrap();
    fs::rename(
        trial.join("OpenCamera/take.mp4"),
        trial.join(format!("OpenCamera/.takedock-delete-{OP}")),
    )
    .unwrap();
}

#[test]
fn claim_crash_restores_source() {
    let (trial, source, target, identity) = two_roots("take.mp4");
    interrupted_intent(trial.path(), &identity);
    source.recover().unwrap();
    assert_eq!(source.stat("take.mp4").unwrap().inode, identity.inode);
    assert_eq!(
        fs::read(trial.path().join("OpenCamera/take.mp4")).unwrap(),
        movie()
    );
    assert!(target.list().unwrap().is_empty());
}

#[test]
fn committed_crash_never_returns_destination() {
    let (trial, source, target, identity) = two_roots("take.mp4");
    interrupted_intent(trial.path(), &identity);
    fs::rename(
        trial
            .path()
            .join(format!("OpenCamera/.takedock-delete-{OP}")),
        trial.path().join("TakeDock/take.mp4"),
    )
    .unwrap();
    source.recover().unwrap();
    assert!(source.list().unwrap().is_empty());
    assert_eq!(target.stat("take.mp4").unwrap().inode, identity.inode);
    assert_eq!(
        fs::read(trial.path().join("TakeDock/take.mp4")).unwrap(),
        movie()
    );
}

#[test]
fn destination_removed_after_commit_never_blocks_source_operations() {
    let (trial, source, _target, identity) = two_roots("take.mp4");
    interrupted_intent(trial.path(), &identity);
    fs::rename(
        trial
            .path()
            .join(format!("OpenCamera/.takedock-delete-{OP}")),
        trial.path().join("TakeDock/take.mp4"),
    )
    .unwrap();
    fs::remove_file(trial.path().join("TakeDock/take.mp4")).unwrap();
    fs::remove_dir(trial.path().join("TakeDock")).unwrap();
    source.recover().unwrap();
    fs::write(trial.path().join("OpenCamera/other.mp4"), movie()).unwrap();
    source
        .rename(
            "other.mp4",
            "remaining.mp4",
            &source.stat("other.mp4").unwrap(),
        )
        .unwrap();
    assert!(source.stat("remaining.mp4").is_ok());
    assert!(!trial.path().join("TakeDock").exists());
}

#[test]
fn legacy_delete_journal_still_recovers() {
    let (trial, source, target, identity) = two_roots("take.mp4");
    interrupted_intent(trial.path(), &identity);
    let path = trial
        .path()
        .join(format!("OpenCamera/.takedock-journal/{OP}.json"));
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    value.as_object_mut().unwrap().remove("handoff");
    fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
    source.recover().unwrap();
    assert_eq!(source.stat("take.mp4").unwrap().inode, identity.inode);
    assert!(target.list().unwrap().is_empty());
}

#[cfg(unix)]
#[test]
fn destination_symlink_is_rejected() {
    let (trial, source, target, identity) = two_roots("take.mp4");
    fs::rename(trial.path().join("TakeDock"), trial.path().join("outside")).unwrap();
    std::os::unix::fs::symlink(trial.path().join("outside"), trial.path().join("TakeDock"))
        .unwrap();
    assert!(Root::open(&trial.path().join("TakeDock")).is_err());
    assert!(matches!(
        source.handoff_to(&target, "take.mp4", &identity, OP, || Ok(true)),
        Outcome::Retained { .. }
    ));
    assert!(trial.path().join("OpenCamera/take.mp4").exists());
}

#[cfg(target_os = "linux")]
#[test]
fn cross_device_destination_never_falls_back_to_copy() {
    let (_trial, source, _target, identity) = two_roots("take.mp4");
    let directory = tempfile::tempdir_in("/dev/shm").unwrap();
    let target = Root::open(directory.path()).unwrap();
    assert!(matches!(
        source.handoff_to(&target, "take.mp4", &identity, OP, || Ok(true)),
        Outcome::Retained { .. }
    ));
    assert_eq!(source.stat("take.mp4").unwrap(), identity);
    assert!(target.list().unwrap().is_empty());
}
