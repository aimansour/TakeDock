//! One-way metadata relocation; callers hold the existing mutation lock.
use crate::{Identity, Journal, Root, err, identity, same_file_after_rename, validate_name};
use serde::{Deserialize, Serialize};
use std::{io::Write, path::Path};

pub const HANDOFF_ROOT: &str = "/sdcard/DCIM/TakeDock";
pub const MAX_MESSAGE: usize = 4096;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub operation_id: String,
    pub source_name: String,
    pub destination_name: String,
    #[serde(with = "receipt_identity")]
    pub identity: Identity,
}

// Internally tagged enum deserialization buffers nested values and cannot
// deserialize u128 fields. Carry the exact identity as opaque JSON text, as
// other host/source identity messages already do, without rounding timestamps.
mod receipt_identity {
    use super::Identity;
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(value: &Identity, output: S) -> Result<S::Ok, S::Error> {
        let wire = serde_json::to_string(value).map_err(serde::ser::Error::custom)?;
        output.serialize_str(&wire)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(input: D) -> Result<Identity, D::Error> {
        let wire = String::deserialize(input)?;
        serde_json::from_str(&wire).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::{
        cell::RefCell,
        fs,
        panic::{AssertUnwindSafe, catch_unwind},
    };
    #[derive(Clone, Copy, PartialEq)]
    pub(crate) enum Point {
        ClaimUnverified,
        Claimed,
        Published,
    }
    type Hook = Option<(Point, Box<dyn FnOnce()>)>;
    thread_local! { static HOOK: RefCell<Hook> = const { RefCell::new(None) }; }
    fn on(point: Point, callback: impl FnOnce() + 'static) {
        HOOK.with(|hook| *hook.borrow_mut() = Some((point, Box::new(callback))));
    }
    pub(crate) fn checkpoint(point: Point) {
        let callback = HOOK.with(|hook| {
            let mut hook = hook.borrow_mut();
            if hook.as_ref().is_some_and(|(wanted, _)| *wanted == point) {
                hook.take().map(|(_, callback)| callback)
            } else {
                None
            }
        });
        if let Some(callback) = callback {
            callback();
        }
    }
    fn fixture() -> (tempfile::TempDir, Root, Root, Identity) {
        let directory = tempfile::tempdir().unwrap();
        fs::create_dir(directory.path().join("source")).unwrap();
        fs::create_dir(directory.path().join("target")).unwrap();
        fn atom(kind: &[u8; 4], bytes: &[u8]) -> Vec<u8> {
            [
                ((bytes.len() + 8) as u32).to_be_bytes().as_slice(),
                kind,
                bytes,
            ]
            .concat()
        }
        let mut timeline = [0; 20];
        timeline[12..16].copy_from_slice(&1000u32.to_be_bytes());
        timeline[16..20].copy_from_slice(&100u32.to_be_bytes());
        let bytes = [
            atom(b"ftyp", b"isom0000"),
            atom(b"mdat", b"take"),
            atom(b"moov", &atom(b"mvhd", &timeline)),
        ]
        .concat();
        fs::write(directory.path().join("source/take.mp4"), bytes).unwrap();
        let source = Root::open(&directory.path().join("source")).unwrap();
        let target = Root::open(&directory.path().join("target")).unwrap();
        let expected = source.stat("take.mp4").unwrap();
        (directory, source, target, expected)
    }
    const OP: &str = "4fb0ef42-f12b-4b61-ae88-cf0bd0650084";
    #[test]
    fn actual_publication_cannot_overwrite_a_file_created_after_claim() {
        let (directory, source, target, expected) = fixture();
        let path = directory.path().join("target/take.mp4");
        on(Point::Claimed, move || {
            fs::write(path, b"user's last-moment target").unwrap()
        });
        let result = source.handoff_to(&target, "take.mp4", &expected, OP, || Ok(true));
        assert!(matches!(result, Outcome::Retained { .. }), "{result:?}");
        assert_eq!(
            fs::read(directory.path().join("target/take.mp4")).unwrap(),
            b"user's last-moment target"
        );
        assert_eq!(source.stat("take.mp4").unwrap().inode, expected.inode);
    }
    #[test]
    fn real_claim_and_publication_crash_boundaries_recover_without_undoing_commit() {
        for point in [Point::Claimed, Point::Published] {
            let (_directory, source, target, expected) = fixture();
            on(point, || panic!("simulated process loss"));
            let result = catch_unwind(AssertUnwindSafe(|| {
                source.handoff_to(&target, "take.mp4", &expected, OP, || Ok(true))
            }));
            assert!(
                result.is_err(),
                "checkpoint must interrupt the actual transaction"
            );
            source.recover().unwrap();
            if point == Point::Claimed {
                assert_eq!(source.stat("take.mp4").unwrap().inode, expected.inode);
                assert!(target.list().unwrap().is_empty());
            } else {
                assert!(source.list().unwrap().is_empty());
                assert_eq!(target.stat("take.mp4").unwrap().inode, expected.inode);
            }
        }
    }
    #[test]
    fn committed_move_stays_successful_when_intent_cleanup_fails() {
        let (directory, source, target, expected) = fixture();
        let journal = directory.path().join("source/.takedock-journal");
        on(Point::Published, move || {
            for entry in fs::read_dir(journal).unwrap() {
                let path = entry.unwrap().path();
                if path
                    .extension()
                    .is_some_and(|extension| extension == "json")
                {
                    fs::remove_file(path).unwrap();
                }
            }
        });
        let result = source.handoff_to(&target, "take.mp4", &expected, OP, || Ok(true));
        assert!(
            matches!(
                result,
                Outcome::HandedOff {
                    warning: Some(_),
                    ..
                }
            ),
            "{result:?}"
        );
        assert_eq!(target.stat("take.mp4").unwrap().inode, expected.inode);
        assert!(source.list().unwrap().is_empty());
    }
    #[test]
    fn failed_claim_validation_does_not_claim_that_the_original_was_retained() {
        let (directory, source, target, expected) = fixture();
        let path = directory.path().join("source");
        on(Point::ClaimUnverified, move || {
            for entry in fs::read_dir(path).unwrap() {
                let path = entry.unwrap().path();
                if path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with(".takedock-delete-")
                {
                    fs::remove_file(path).unwrap();
                }
            }
        });
        let result = source.handoff_to(&target, "take.mp4", &expected, OP, || Ok(true));
        assert!(
            matches!(result, Outcome::OutcomeUnknown { .. }),
            "{result:?}"
        );
    }
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum Outcome {
    HandedOff {
        receipt: Receipt,
        warning: Option<String>,
    },
    Retained {
        code: String,
    },
    OutcomeUnknown {
        code: String,
    },
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Message {
    Prepared {
        operation_id: String,
    },
    Permit {
        operation_id: String,
        allow: bool,
    },
    Result {
        operation_id: String,
        outcome: Outcome,
    },
}
#[derive(Serialize, Deserialize)]
pub(super) struct Intent {
    pub operation_id: String,
    pub destination_name: String,
}
fn retained(code: impl Into<String>) -> Outcome {
    Outcome::Retained { code: code.into() }
}
fn unknown(code: impl Into<String>) -> Outcome {
    Outcome::OutcomeUnknown { code: code.into() }
}

pub fn write_message(output: &mut impl Write, message: &Message) -> Result<(), String> {
    let bytes = serde_json::to_vec(message).map_err(err)?;
    if bytes.len() > MAX_MESSAGE {
        return Err("handoff_message_too_large".into());
    }
    output
        .write_all(&bytes)
        .and_then(|_| output.write_all(b"\n"))
        .and_then(|_| output.flush())
        .map_err(err)
}
pub fn read_permit_stdin() -> Result<Vec<u8>, String> {
    crate::guarded::read_permit_stdin().map_err(|error| {
        if error.ends_with("timeout") {
            "handoff_permit_timeout"
        } else {
            "handoff_permit_failed"
        }
        .into()
    })
}
pub fn serve(
    source: &Root,
    destination: &Root,
    name: &str,
    expected: &Identity,
    operation_id: &str,
    read_permit: impl FnOnce() -> Result<Vec<u8>, String>,
    output: &mut impl Write,
) -> Result<(), String> {
    let outcome = source.handoff_to(destination, name, expected, operation_id, || {
        write_message(
            output,
            &Message::Prepared {
                operation_id: operation_id.into(),
            },
        )?;
        let bytes = read_permit()?;
        if bytes.len() > MAX_MESSAGE {
            return Err("handoff_permit_failed".into());
        }
        match serde_json::from_slice::<Message>(&bytes).map_err(|_| "handoff_permit_failed")? {
            Message::Permit {
                operation_id: supplied,
                allow,
            } if supplied == operation_id => Ok(allow),
            _ => Err("handoff_permit_failed".into()),
        }
    });
    write_message(
        output,
        &Message::Result {
            operation_id: operation_id.into(),
            outcome,
        },
    )
}

/// Only an explicit handoff creates this folder. Listing and recovery never do.
pub fn open_destination() -> Result<Root, String> {
    let parent = Root::open(Path::new("/sdcard/DCIM"))?;
    #[cfg(unix)]
    {
        use std::os::fd::{AsRawFd, FromRawFd};
        let name = std::ffi::CString::new("TakeDock").unwrap();
        if unsafe { libc::mkdirat(parent.directory.as_raw_fd(), name.as_ptr(), 0o700) } != 0
            && std::io::Error::last_os_error().kind() != std::io::ErrorKind::AlreadyExists
        {
            return Err(err(std::io::Error::last_os_error()));
        }
        let fd = unsafe {
            libc::openat(
                parent.directory.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(err(std::io::Error::last_os_error()));
        }
        let root = Root {
            path: HANDOFF_ROOT.into(),
            directory: unsafe { std::fs::File::from_raw_fd(fd) },
        };
        parent.ensure_current()?;
        root.ensure_current()?;
        Ok(root)
    }
    #[cfg(windows)]
    {
        match std::fs::create_dir(HANDOFF_ROOT) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(err(error)),
        }
        parent.ensure_current()?;
        Root::open(Path::new(HANDOFF_ROOT))
    }
}

impl Root {
    fn ensure_current(&self) -> Result<(), String> {
        let current = Root::open(&self.path)?;
        let opened = identity(&self.directory)?;
        let actual = identity(&current.directory)?;
        if opened.device != actual.device || opened.inode != actual.inode {
            return Err("handoff_directory_changed".into());
        }
        Ok(())
    }
    fn entry_exists(&self, name: &str) -> Result<bool, String> {
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            let name = std::ffi::CString::new(name).map_err(err)?;
            let mut info: libc::stat = unsafe { std::mem::zeroed() };
            if unsafe {
                libc::fstatat(
                    self.directory.as_raw_fd(),
                    name.as_ptr(),
                    &mut info,
                    libc::AT_SYMLINK_NOFOLLOW,
                )
            } == 0
            {
                return Ok(true);
            }
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::NotFound {
                Ok(false)
            } else {
                Err(err(error))
            }
        }
        #[cfg(windows)]
        {
            match std::fs::symlink_metadata(self.path.join(name)) {
                Ok(_) => Ok(true),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
                Err(error) => Err(err(error)),
            }
        }
    }
    fn handoff_ready(
        &self,
        target: &Root,
        name: &str,
        expected: &Identity,
        operation: &str,
    ) -> Result<(), String> {
        validate_name(name)?;
        uuid::Uuid::parse_str(operation).map_err(|_| "handoff_invalid_operation")?;
        self.ensure_current()?;
        target.ensure_current()?;
        let source_id = identity(&self.directory)?;
        let target_id = identity(&target.directory)?;
        if source_id.device != target_id.device {
            return Err("handoff_cross_device".into());
        }
        if source_id.inode == target_id.inode {
            return Err("handoff_same_directory".into());
        }
        self.checked(name, expected)?;
        if !self.video_ready(name)? {
            return Err("video_not_finalized".into());
        }
        self.checked(name, expected)?;
        if target.entry_exists(name)? {
            return Err("destination_exists".into());
        }
        Ok(())
    }
    fn restore_handoff(
        &self,
        journal: &Journal,
        directory: &Root,
        filename: &str,
        code: String,
    ) -> Outcome {
        if self.restore(journal).is_err() {
            return unknown("handoff_recovery_required");
        }
        let _ = directory.remove_raw(filename);
        match self.stat(&journal.name) {
            Ok(actual) if same_file_after_rename(&actual, &journal.identity) => retained(code),
            _ => unknown("handoff_recovery_required"),
        }
    }
    pub fn handoff_to(
        &self,
        target: &Root,
        name: &str,
        expected: &Identity,
        operation_id: &str,
        permit: impl FnOnce() -> Result<bool, String>,
    ) -> Outcome {
        if let Err(error) = self.handoff_ready(target, name, expected, operation_id) {
            return retained(error);
        }
        match permit() {
            Ok(true) => {}
            Ok(false) => return retained("operation_cancelled"),
            Err(error) => return retained(error),
        }
        if let Err(error) = self.handoff_ready(target, name, expected, operation_id) {
            return retained(error);
        }
        let (journal, directory, filename) = match self.claim_for(
            name,
            expected,
            Some(Intent {
                operation_id: operation_id.into(),
                destination_name: name.into(),
            }),
        ) {
            Ok(claim) => claim,
            Err(error) => {
                return match self.stat(name) {
                    Ok(actual) if same_file_after_rename(&actual, expected) => retained(error),
                    _ => unknown("handoff_recovery_required"),
                };
            }
        };
        #[cfg(test)]
        tests::checkpoint(tests::Point::Claimed);
        if let Err(error) = self
            .ensure_current()
            .and_then(|_| target.ensure_current())
            .and_then(|_| {
                let actual = identity(&self.open_named(&journal.claim)?)?;
                if same_file_after_rename(&actual, expected) {
                    Ok(())
                } else {
                    Err("source_changed".into())
                }
            })
        {
            return self.restore_handoff(&journal, &directory, &filename, error);
        }
        if let Err(error) = self.move_to_raw(&journal.claim, target, name) {
            return self.restore_handoff(&journal, &directory, &filename, error);
        }
        #[cfg(test)]
        tests::checkpoint(tests::Point::Published);
        // Published destination is never undone, including on lost acknowledgement.
        let actual = match target.ensure_current().and_then(|_| target.stat(name)) {
            Ok(actual) if same_file_after_rename(&actual, expected) => actual,
            _ => return unknown("handoff_outcome_unknown"),
        };
        let warning = directory
            .remove_raw(&filename)
            .err()
            .map(|_| "handoff_cleanup_pending".into());
        Outcome::HandedOff {
            receipt: Receipt {
                operation_id: operation_id.into(),
                source_name: name.into(),
                destination_name: name.into(),
                identity: actual,
            },
            warning,
        }
    }
}
