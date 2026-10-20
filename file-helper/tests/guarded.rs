use std::{cell::Cell, fs};
use takedock_files::{Root, guarded};

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

#[test]
fn rejected_permit_keeps_original_and_creates_no_claim() {
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("take.mp4"), movie()).unwrap();
    let root = Root::open(directory.path()).unwrap();
    let _lock = root.lock().unwrap();
    let identity = root.stat("take.mp4").unwrap();
    assert!(
        root.delete_guarded("take.mp4", &identity, || Ok(false))
            .is_err()
    );
    assert_eq!(root.stat("take.mp4").unwrap(), identity);
    assert!(!fs::read_dir(directory.path()).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".takedock-delete-")
    }));
}

#[test]
fn source_replaced_during_preparation_is_retained() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("take.mp4");
    fs::write(&path, movie()).unwrap();
    let root = Root::open(directory.path()).unwrap();
    let _lock = root.lock().unwrap();
    let expected = root.stat("take.mp4").unwrap();
    let result = root.delete_guarded("take.mp4", &expected, || {
        fs::rename(&path, directory.path().join("original.mp4")).unwrap();
        fs::write(&path, b"replacement").unwrap();
        Ok(true)
    });
    assert!(result.is_err());
    assert_eq!(fs::read(path).unwrap(), b"replacement");
    assert_eq!(
        fs::read(directory.path().join("original.mp4")).unwrap(),
        movie()
    );
}

#[test]
fn authorized_delete_removes_only_the_finalized_exact_source() {
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("take.mp4"), movie()).unwrap();
    fs::write(directory.path().join("other.mp4"), b"unrelated").unwrap();
    let root = Root::open(directory.path()).unwrap();
    let _lock = root.lock().unwrap();
    let expected = root.stat("take.mp4").unwrap();
    root.delete_guarded("take.mp4", &expected, || Ok(true))
        .unwrap();
    assert!(!directory.path().join("take.mp4").exists());
    assert_eq!(
        fs::read(directory.path().join("other.mp4")).unwrap(),
        b"unrelated"
    );
}

#[test]
fn unfinished_or_unsafe_source_never_requests_permission() {
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("take.mp4"), b"unfinished").unwrap();
    let root = Root::open(directory.path()).unwrap();
    let identity = root.stat("take.mp4").unwrap();
    let called = Cell::new(false);
    assert!(
        root.delete_guarded("take.mp4", &identity, || {
            called.set(true);
            Ok(true)
        })
        .is_err()
    );
    assert!(
        root.delete_guarded("../take.mp4", &identity, || {
            called.set(true);
            Ok(true)
        })
        .is_err()
    );
    assert!(!called.get());
}

#[test]
fn permit_eof_wrong_operation_and_timeout_never_delete() {
    for permit in [
        Ok(Vec::new()),
        Ok(br#"{"type":"permit","operation_id":"wrong","allow":true}"#.to_vec()),
        Err("discard_permit_timeout".into()),
    ] {
        let directory = tempfile::tempdir().unwrap();
        fs::write(directory.path().join("take.mp4"), movie()).unwrap();
        let root = Root::open(directory.path()).unwrap();
        let _lock = root.lock().unwrap();
        let identity = root.stat("take.mp4").unwrap();
        let mut output = Vec::new();
        guarded::serve(&root, "take.mp4", &identity, "op-1", || permit, &mut output).unwrap();
        assert_eq!(root.stat("take.mp4").unwrap(), identity);
        let messages = String::from_utf8(output).unwrap();
        let result: serde_json::Value =
            serde_json::from_str(messages.lines().last().unwrap()).unwrap();
        assert_eq!(result["type"], "result");
        assert_eq!(result["status"], "retained");
    }
}
