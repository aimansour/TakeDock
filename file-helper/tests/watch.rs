use std::fs;
use takedock_files::{
    Root,
    watch::{WatchBody, decode_frame},
};

fn movie() -> Vec<u8> {
    fn atom(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut bytes = ((payload.len() + 8) as u32).to_be_bytes().to_vec();
        bytes.extend(kind);
        bytes.extend(payload);
        bytes
    }
    let mut timeline = [0; 20];
    timeline[12..16].copy_from_slice(&1000u32.to_be_bytes());
    timeline[16..20].copy_from_slice(&100u32.to_be_bytes());
    [
        atom(b"ftyp", b"isom0000"),
        atom(b"mdat", b"video"),
        atom(b"moov", &atom(b"mvhd", &timeline)),
    ]
    .concat()
}

#[test]
fn watch_protocol_rejects_truncation_unknown_version_and_oversized_frame() {
    assert!(decode_frame(b"{\"version\":1").is_err());
    assert!(
        decode_frame(br#"{"version":2,"stream":"s","sequence":1,"body":{"type":"heartbeat"}}"#)
            .is_err()
    );
    assert!(decode_frame(&vec![b' '; 65_537]).is_err());
    assert!(
        decode_frame(br#"{"version":1,"stream":"","sequence":1,"body":{"type":"heartbeat"}}"#)
            .is_err()
    );
    let frame =
        decode_frame(br#"{"version":1,"stream":"s","sequence":1,"body":{"type":"heartbeat"}}"#)
            .unwrap();
    assert_eq!((frame.stream.as_str(), frame.sequence), ("s", 1));
    assert!(matches!(frame.body, WatchBody::Heartbeat));
}

#[test]
fn inspect_and_manifest_do_not_take_the_mutation_lock() {
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("رحلة.mp4"), movie()).unwrap();
    let root = Root::open(directory.path()).unwrap();
    let _mutation_lock = root.lock().unwrap();
    let inspector = Root::open(directory.path()).unwrap();
    let entry = inspector.inspect("رحلة.mp4").unwrap();
    assert!(entry.ready);
    assert_eq!(entry.duration_ms, Some(100));
    assert_eq!(entry.identity, root.stat("رحلة.mp4").unwrap());
    let manifest = inspector.manifest().unwrap();
    assert_eq!(manifest.len(), 1);
    assert_eq!(manifest[0].name, "رحلة.mp4");
    assert_eq!(manifest[0].identity, entry.identity);
}

#[test]
fn inspect_reports_changed_identity_and_never_invents_finalization() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("take.mp4");
    fs::write(&path, movie()).unwrap();
    let root = Root::open(directory.path()).unwrap();
    let old = root.stat("take.mp4").unwrap();
    fs::write(path, b"unfinished").unwrap();
    let entry = root.inspect("take.mp4").unwrap();
    assert_ne!(entry.identity, old);
    assert!(!entry.ready);
    assert_eq!(entry.duration_ms, None);
    assert!(root.inspect("../take.mp4").is_err());
}

#[cfg(unix)]
#[test]
fn inspect_and_manifest_reject_symlink_sources() {
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("original.mp4"), movie()).unwrap();
    std::os::unix::fs::symlink("original.mp4", directory.path().join("link.mp4")).unwrap();
    let root = Root::open(directory.path()).unwrap();
    assert!(root.inspect("link.mp4").is_err());
    assert!(root.manifest().is_err());
}

#[cfg(any(target_os = "linux", target_os = "android"))]
mod native {
    use super::*;
    use std::{
        io::{self, Write},
        sync::mpsc,
        thread,
        time::Duration,
    };
    use takedock_files::watch::{self, WatchFrame};

    struct Lines {
        buffer: Vec<u8>,
        sender: mpsc::Sender<WatchFrame>,
    }
    impl Write for Lines {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.buffer.extend(bytes);
            while let Some(end) = self.buffer.iter().position(|byte| *byte == b'\n') {
                let line: Vec<_> = self.buffer.drain(..=end).collect();
                let frame = serde_json::from_slice(&line).unwrap();
                self.sender
                    .send(frame)
                    .map_err(|_| io::ErrorKind::BrokenPipe)?;
            }
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    fn next(
        receiver: &mpsc::Receiver<WatchFrame>,
        predicate: impl Fn(&WatchBody) -> bool,
    ) -> WatchFrame {
        loop {
            let frame = receiver.recv_timeout(Duration::from_secs(5)).unwrap();
            if predicate(&frame.body) {
                return frame;
            }
        }
    }

    #[test]
    fn watch_does_not_take_the_mutation_lock() {
        let directory = tempfile::tempdir().unwrap();
        let root = Root::open(directory.path()).unwrap();
        let _lock = root.lock().unwrap();
        let path = directory.path().to_owned();
        let (sender, receiver) = mpsc::channel();
        let worker = thread::spawn(move || {
            watch::run(
                &Root::open(&path).unwrap(),
                "s",
                &mut Lines {
                    buffer: Vec::new(),
                    sender,
                },
            )
        });
        next(&receiver, |body| matches!(body, WatchBody::BaselineEnd));
        drop(receiver);
        assert!(worker.join().unwrap().is_err());
    }

    #[test]
    fn linux_watch_reports_creation_close_and_root_loss() {
        let parent = tempfile::tempdir().unwrap();
        let path = parent.path().join("videos");
        fs::create_dir(&path).unwrap();
        let watching = path.clone();
        let (sender, receiver) = mpsc::channel();
        let worker = thread::spawn(move || {
            watch::run(
                &Root::open(&watching).unwrap(),
                "s",
                &mut Lines {
                    buffer: Vec::new(),
                    sender,
                },
            )
        });
        next(&receiver, |body| matches!(body, WatchBody::BaselineEnd));
        fs::write(path.join("new.mp4"), movie()).unwrap();
        let created = next(&receiver, |body| matches!(body, WatchBody::Created { .. }));
        assert!(matches!(created.body, WatchBody::Created { entry } if entry.name == "new.mp4"));
        next(&receiver, |body| matches!(body, WatchBody::Closed { .. }));
        fs::rename(path, parent.path().join("moved")).unwrap();
        next(&receiver, |body| matches!(body, WatchBody::Lost { .. }));
        assert!(worker.join().unwrap().is_err());
    }

    #[test]
    fn baseline_race_never_publishes_a_complete_manifest() {
        struct Race {
            lines: Vec<u8>,
            root: std::path::PathBuf,
            raced: bool,
            resets: usize,
            included: bool,
        }
        impl Write for Race {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                self.lines.extend(bytes);
                while let Some(end) = self.lines.iter().position(|byte| *byte == b'\n') {
                    let line: Vec<_> = self.lines.drain(..=end).collect();
                    let frame: WatchFrame = serde_json::from_slice(&line).unwrap();
                    match frame.body {
                        WatchBody::BaselineBegin => {
                            self.resets += 1;
                            self.included = false;
                        }
                        WatchBody::BaselineEntry { entry } => {
                            if entry.name == "raced.mp4" {
                                self.included = true;
                            }
                            if !self.raced {
                                self.raced = true;
                                fs::write(self.root.join("raced.mp4"), movie()).unwrap();
                            }
                        }
                        WatchBody::BaselineEnd => {
                            assert!(
                                self.included,
                                "completed a baseline that missed its concurrent addition"
                            );
                            return Err(io::ErrorKind::BrokenPipe.into());
                        }
                        WatchBody::Lost { .. } => return Err(io::ErrorKind::BrokenPipe.into()),
                        _ => {}
                    }
                }
                Ok(bytes.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let directory = tempfile::tempdir().unwrap();
        fs::write(directory.path().join("old.mp4"), movie()).unwrap();
        let root = Root::open(directory.path()).unwrap();
        let mut output = Race {
            lines: Vec::new(),
            root: directory.path().into(),
            raced: false,
            resets: 0,
            included: false,
        };
        assert!(watch::run(&root, "s", &mut output).is_err());
        assert!(output.raced);
    }
}
