use serde::{Deserialize, Serialize};
mod finalized;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub const VIDEO_ROOT: &str = "/sdcard/DCIM/OpenCamera";
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity {
    pub device: u64,
    pub inode: u64,
    pub size: u64,
    pub modified_ns: u128,
    pub changed_ns: u128,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub name: String,
    pub identity: Identity,
    pub ready: bool,
}
pub struct Root {
    path: PathBuf,
    #[cfg(unix)]
    directory: File,
}
#[derive(Serialize, Deserialize)]
struct Journal {
    version: u32,
    name: String,
    claim: String,
    identity: Identity,
}
impl Root {
    pub fn open(path: &Path) -> Result<Self, String> {
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC);
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            use windows_sys::Win32::Storage::FileSystem::*;
            options.custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT);
        }
        if fs::symlink_metadata(path)
            .map_err(err)?
            .file_type()
            .is_symlink()
        {
            return Err("symlink_directory".into());
        }
        let directory = options.open(path).map_err(err)?;
        if !directory.metadata().map_err(err)?.is_dir() {
            return Err("not_a_directory".into());
        }
        Ok(Self {
            path: path.into(),
            #[cfg(unix)]
            directory,
        })
    }
    fn open_named(&self, name: &str) -> Result<File, String> {
        #[cfg(unix)]
        {
            use std::os::fd::{AsRawFd, FromRawFd};
            let name = std::ffi::CString::new(name).map_err(err)?;
            let fd = unsafe {
                libc::openat(
                    self.directory.as_raw_fd(),
                    name.as_ptr(),
                    libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                )
            };
            if fd < 0 {
                return Err(err(std::io::Error::last_os_error()));
            }
            let file = unsafe { File::from_raw_fd(fd) };
            if !file.metadata().map_err(err)?.is_file() {
                return Err("not_a_regular_file".into());
            }
            Ok(file)
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
            use windows_sys::Win32::Storage::FileSystem::*;
            let file = OpenOptions::new()
                .read(true)
                .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
                .open(self.path.join(name))
                .map_err(err)?;
            let metadata = file.metadata().map_err(err)?;
            if !metadata.is_file() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
            {
                return Err("not_a_regular_file".into());
            }
            Ok(file)
        }
    }
    pub fn list(&self) -> Result<Vec<Entry>, String> {
        let mut entries = Vec::new();
        for entry in fs::read_dir(&self.path).map_err(err)? {
            let entry = entry.map_err(err)?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| "filename_encoding")?;
            if name.starts_with(".takedock-") {
                continue;
            }
            let metadata = entry.file_type().map_err(err)?;
            if metadata.is_file() {
                entries.push(Entry {
                    identity: self.stat(&name)?,
                    ready: self.video_ready(&name)?,
                    name,
                });
            }
        }
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(entries)
    }
    pub fn stat(&self, name: &str) -> Result<Identity, String> {
        validate_name(name)?;
        identity(&self.open_named(name)?)
    }
    fn checked(&self, name: &str, expected: &Identity) -> Result<File, String> {
        validate_name(name)?;
        let file = self.open_named(name)?;
        if identity(&file)? != *expected {
            return Err("source_changed".into());
        }
        Ok(file)
    }
    fn rename_raw(&self, source: &str, destination: &str) -> Result<(), String> {
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            let source = std::ffi::CString::new(source).map_err(err)?;
            let destination = std::ffi::CString::new(destination).map_err(err)?;
            let result = unsafe {
                libc::syscall(
                    libc::SYS_renameat2,
                    self.directory.as_raw_fd(),
                    source.as_ptr(),
                    self.directory.as_raw_fd(),
                    destination.as_ptr(),
                    1u32,
                )
            };
            if result != 0 {
                return Err(err(std::io::Error::last_os_error()));
            }
            Ok(())
        }
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            use windows_sys::Win32::Storage::FileSystem::MoveFileExW;
            let source: Vec<u16> = self
                .path
                .join(source)
                .as_os_str()
                .encode_wide()
                .chain(Some(0))
                .collect();
            let destination: Vec<u16> = self
                .path
                .join(destination)
                .as_os_str()
                .encode_wide()
                .chain(Some(0))
                .collect();
            if unsafe { MoveFileExW(source.as_ptr(), destination.as_ptr(), 0) } == 0 {
                return Err(err(std::io::Error::last_os_error()));
            }
            Ok(())
        }
    }
    fn remove_raw(&self, name: &str) -> Result<(), String> {
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            let name = std::ffi::CString::new(name).map_err(err)?;
            if unsafe { libc::unlinkat(self.directory.as_raw_fd(), name.as_ptr(), 0) } != 0 {
                return Err(err(std::io::Error::last_os_error()));
            }
            Ok(())
        }
        #[cfg(windows)]
        {
            fs::remove_file(self.path.join(name)).map_err(err)
        }
    }
    fn journal_directory(&self) -> Result<Self, String> {
        let path = self.path.join(".takedock-journal");
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            let name = std::ffi::CString::new(".takedock-journal").unwrap();
            if unsafe { libc::mkdirat(self.directory.as_raw_fd(), name.as_ptr(), 0o700) } != 0
                && std::io::Error::last_os_error().kind() != std::io::ErrorKind::AlreadyExists
            {
                return Err(err(std::io::Error::last_os_error()));
            }
        }
        #[cfg(windows)]
        {
            match fs::create_dir(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(err(error)),
            }
        }
        Self::open(&path)
    }
    fn create_named(&self, name: &str) -> Result<File, String> {
        #[cfg(unix)]
        {
            use std::os::fd::{AsRawFd, FromRawFd};
            let name = std::ffi::CString::new(name).map_err(err)?;
            let fd = unsafe {
                libc::openat(
                    self.directory.as_raw_fd(),
                    name.as_ptr(),
                    libc::O_WRONLY
                        | libc::O_CREAT
                        | libc::O_EXCL
                        | libc::O_NOFOLLOW
                        | libc::O_CLOEXEC,
                    0o600,
                )
            };
            if fd < 0 {
                return Err(err(std::io::Error::last_os_error()));
            }
            Ok(unsafe { File::from_raw_fd(fd) })
        }
        #[cfg(windows)]
        {
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(self.path.join(name))
                .map_err(err)
        }
    }
    fn restore(&self, journal: &Journal) -> Result<(), String> {
        if self.rename_raw(&journal.claim, &journal.name).is_ok() {
            return Ok(());
        }
        let extension = Path::new(&journal.name)
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("mp4");
        self.rename_raw(
            &journal.claim,
            &format!("recovered-{}.{}", uuid::Uuid::new_v4(), extension),
        )
    }
    fn claim(&self, name: &str, expected: &Identity) -> Result<(Journal, Self, String), String> {
        self.checked(name, expected)?;
        let journal_directory = self.journal_directory()?;
        let token = uuid::Uuid::new_v4().to_string();
        let journal = Journal {
            version: 1,
            name: name.into(),
            claim: format!(".takedock-delete-{token}"),
            identity: expected.clone(),
        };
        let filename = format!("{token}.json");
        let mut file = journal_directory.create_named(&filename)?;
        file.write_all(&serde_json::to_vec(&journal).map_err(err)?)
            .map_err(err)?;
        file.sync_all().map_err(err)?;
        drop(file);
        self.rename_raw(name, &journal.claim)?;
        let actual = identity(&self.open_named(&journal.claim)?)?;
        if !same_file_after_rename(&actual, expected) {
            self.restore(&journal)?;
            journal_directory.remove_raw(&filename)?;
            return Err("source_changed".into());
        }
        Ok((journal, journal_directory, filename))
    }
    pub fn rename(
        &self,
        source: &str,
        destination: &str,
        expected: &Identity,
    ) -> Result<(), String> {
        validate_name(destination)?;
        if source == destination {
            self.checked(source, expected)?;
            return Ok(());
        }
        let (journal, directory, filename) = self.claim(source, expected)?;
        if let Err(error) = self.rename_raw(&journal.claim, destination) {
            self.restore(&journal)?;
            directory.remove_raw(&filename)?;
            return Err(error);
        }
        directory.remove_raw(&filename)?;
        Ok(())
    }
    pub fn delete(&self, name: &str, expected: &Identity) -> Result<(), String> {
        let (journal, directory, filename) = self.claim(name, expected)?;
        if let Err(error) = self.remove_raw(&journal.claim) {
            self.restore(&journal)?;
            directory.remove_raw(&filename)?;
            return Err(error);
        }
        directory.remove_raw(&filename)?;
        Ok(())
    }
    pub fn hash(&self, name: &str, expected: &Identity) -> Result<String, String> {
        let mut file = self.checked(name, expected)?;
        let mut hasher = Sha256::new();
        let mut buffer = [0; 65536];
        loop {
            let size = file.read(&mut buffer).map_err(err)?;
            if size == 0 {
                break;
            }
            hasher.update(&buffer[..size]);
        }
        if identity(&file)? != *expected {
            return Err("source_changed".into());
        }
        Ok(hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect())
    }
    pub fn read(
        &self,
        name: &str,
        expected: &Identity,
        output: &mut impl Write,
    ) -> Result<(), String> {
        let mut file = self.checked(name, expected)?;
        std::io::copy(&mut file, output).map_err(err)?;
        if identity(&file)? != *expected {
            return Err("source_changed".into());
        }
        Ok(())
    }
    pub fn recover(&self) -> Result<(), String> {
        let path = self.path.join(".takedock-journal");
        if !path.exists() {
            return Ok(());
        }
        let directory = Self::open(&path)?;
        for entry in fs::read_dir(&path).map_err(err)? {
            let entry = entry.map_err(err)?;
            let filename = entry
                .file_name()
                .into_string()
                .map_err(|_| "filename_encoding")?;
            if filename == ".lock" {
                continue;
            }
            let token = filename.strip_suffix(".json").ok_or("invalid_journal")?;
            uuid::Uuid::parse_str(token).map_err(err)?;
            let mut file = directory.open_named(&filename)?;
            let mut bytes = Vec::new();
            Read::by_ref(&mut file)
                .take(8192)
                .read_to_end(&mut bytes)
                .map_err(err)?;
            let journal: Journal = serde_json::from_slice(&bytes).map_err(err)?;
            if journal.version != 1 || journal.claim != format!(".takedock-delete-{token}") {
                return Err("invalid_journal".into());
            }
            validate_name(&journal.name)?;
            if let Ok(file) = self.open_named(&journal.claim) {
                if !same_file_after_rename(&identity(&file)?, &journal.identity) {
                    return Err("recovery_source_changed".into());
                }
                drop(file);
                self.restore(&journal)?;
            }
            directory.remove_raw(&filename)?;
        }
        Ok(())
    }
    pub fn lock(&self) -> Result<File, String> {
        #[cfg(not(target_os = "android"))]
        let journal = self.journal_directory()?;
        #[cfg(not(target_os = "android"))]
        let handle = match journal.create_named(".lock") {
            Ok(handle) => handle,
            Err(_) => journal.open_named(".lock")?,
        };
        #[cfg(target_os = "android")]
        let handle = {
            use std::os::fd::AsRawFd;
            use std::os::unix::fs::OpenOptionsExt;
            // Android's shared-storage FUSE does not support flock. This shell-owned
            // lock lives on private storage; the journal remains alongside videos.
            let handle = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
                .open("/data/local/tmp/takedock-files.lock")
                .map_err(err)?;
            // std::fs::File::lock does not implement Android. Bionic exposes flock.
            if unsafe { libc::flock(handle.as_raw_fd(), libc::LOCK_EX) } != 0 {
                return Err(err(std::io::Error::last_os_error()));
            }
            handle
        };
        #[cfg(not(target_os = "android"))]
        handle.lock().map_err(err)?;
        Ok(handle)
    }
    pub fn video_ready(&self, name: &str) -> Result<bool, String> {
        validate_name(name)?;
        let mut file = self.open_named(name)?;
        let before = identity(&file)?;
        let ready = finalized::finalized(&mut file, before.size);
        Ok(ready && identity(&file)? == before)
    }
}
pub fn validate_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains('/')
        || name.contains('\0')
        || name.starts_with(".takedock-")
    {
        return Err("unsafe_source_name".into());
    }
    Ok(())
}
fn err(error: impl std::fmt::Display) -> String {
    error.to_string()
}
fn same_file_after_rename(a: &Identity, b: &Identity) -> bool {
    a.device == b.device && a.inode == b.inode && a.size == b.size && a.modified_ns == b.modified_ns
}
fn identity(file: &File) -> Result<Identity, String> {
    let metadata = file.metadata().map_err(err)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(Identity {
            device: metadata.dev(),
            inode: metadata.ino(),
            size: metadata.len(),
            modified_ns: (metadata.mtime() as u128) * 1_000_000_000 + metadata.mtime_nsec() as u128,
            changed_ns: (metadata.ctime() as u128) * 1_000_000_000 + metadata.ctime_nsec() as u128,
        })
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::*;
        let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
        if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
            return Err(err(std::io::Error::last_os_error()));
        }
        let modified = metadata
            .modified()
            .map_err(err)?
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(err)?
            .as_nanos();
        Ok(Identity {
            device: info.dwVolumeSerialNumber as u64,
            inode: ((info.nFileIndexHigh as u64) << 32) | info.nFileIndexLow as u64,
            size: metadata.len(),
            modified_ns: modified,
            changed_ns: modified,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn root() -> (tempfile::TempDir, Root) {
        let dir = tempfile::tempdir().unwrap();
        let root = Root::open(dir.path()).unwrap();
        (dir, root)
    }
    #[test]
    fn path_components_cannot_escape_the_root() {
        for name in [
            "",
            ".",
            "..",
            "../outside.mp4",
            "/absolute.mp4",
            "nul\0byte",
        ] {
            assert!(validate_name(name).is_err(), "{name:?}");
        }
        assert!(validate_name("apostrophe's\nUnicode-video.mp4").is_ok());
    }
    #[test]
    fn listing_preserves_names_and_excludes_nonregular_files() {
        let (dir, root) = root();
        let name = if cfg!(windows) {
            "a'b-café.mp4"
        } else {
            "a'b\n.mp4"
        };
        std::fs::write(dir.path().join(name), b"abc").unwrap();
        std::fs::create_dir(dir.path().join("directory.mp4")).unwrap();
        let entries = root.list().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, name);
        assert_eq!(entries[0].identity.size, 3);
    }
    #[test]
    fn rename_never_overwrites_an_existing_destination() {
        let (dir, root) = root();
        std::fs::write(dir.path().join("source.mp4"), b"source").unwrap();
        std::fs::write(dir.path().join("target.mp4"), b"target").unwrap();
        let id = root.stat("source.mp4").unwrap();
        assert!(root.rename("source.mp4", "target.mp4", &id).is_err());
        assert_eq!(
            std::fs::read(dir.path().join("target.mp4")).unwrap(),
            b"target"
        );
        assert_eq!(
            std::fs::read(dir.path().join("source.mp4")).unwrap(),
            b"source"
        );
    }
    #[test]
    fn rename_updates_the_name_without_copying_or_losing_bytes() {
        let (dir, root) = root();
        std::fs::write(dir.path().join("source.mp4"), b"source").unwrap();
        let id = root.stat("source.mp4").unwrap();
        root.rename("source.mp4", "new.mp4", &id).unwrap();
        assert!(!dir.path().join("source.mp4").exists());
        assert_eq!(root.stat("new.mp4").unwrap().inode, id.inode);
    }
    #[test]
    fn changed_sources_are_retained_on_delete_or_read() {
        let (dir, root) = root();
        std::fs::write(dir.path().join("source.mp4"), b"before").unwrap();
        let id = root.stat("source.mp4").unwrap();
        std::fs::write(dir.path().join("source.mp4"), b"replacement-data").unwrap();
        assert!(root.delete("source.mp4", &id).is_err());
        assert!(root.read("source.mp4", &id, &mut Vec::new()).is_err());
        assert!(dir.path().join("source.mp4").exists());
    }
    #[test]
    fn content_hash_uses_the_open_verified_file() {
        let (dir, root) = root();
        std::fs::write(dir.path().join("source.mp4"), b"abc").unwrap();
        let id = root.stat("source.mp4").unwrap();
        assert_eq!(
            root.hash("source.mp4", &id).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let mut bytes = Vec::new();
        root.read("source.mp4", &id, &mut bytes).unwrap();
        assert_eq!(bytes, b"abc");
    }
    #[test]
    fn verified_delete_removes_only_the_selected_file() {
        let (dir, root) = root();
        std::fs::write(dir.path().join("source.mp4"), b"source").unwrap();
        std::fs::write(dir.path().join("other.mp4"), b"other").unwrap();
        root.delete("source.mp4", &root.stat("source.mp4").unwrap())
            .unwrap();
        assert!(!dir.path().join("source.mp4").exists());
        assert_eq!(
            std::fs::read(dir.path().join("other.mp4")).unwrap(),
            b"other"
        );
    }
    #[cfg(unix)]
    #[test]
    fn symlinks_are_never_read_or_mutated() {
        let (dir, root) = root();
        std::fs::write(dir.path().join("real.mp4"), b"real").unwrap();
        std::os::unix::fs::symlink("real.mp4", dir.path().join("link.mp4")).unwrap();
        assert!(root.stat("link.mp4").is_err());
        assert_eq!(root.list().unwrap().len(), 1);
    }
    #[test]
    fn interrupted_delete_restores_the_claimed_original() {
        let (dir, root) = root();
        std::fs::write(dir.path().join("source.mp4"), b"original").unwrap();
        let identity = root.stat("source.mp4").unwrap();
        let claim = ".takedock-delete-00000000-0000-0000-0000-000000000001";
        std::fs::create_dir(dir.path().join(".takedock-journal")).unwrap();
        let journal =
            serde_json::json!({"version":1,"name":"source.mp4","claim":claim,"identity":identity});
        std::fs::write(
            dir.path()
                .join(".takedock-journal/00000000-0000-0000-0000-000000000001.json"),
            journal.to_string(),
        )
        .unwrap();
        std::fs::rename(dir.path().join("source.mp4"), dir.path().join(claim)).unwrap();
        root.recover().unwrap();
        assert_eq!(
            std::fs::read(dir.path().join("source.mp4")).unwrap(),
            b"original"
        );
        assert!(!dir.path().join(claim).exists());
    }
    #[test]
    fn helper_invocations_share_an_exclusive_root_lock() {
        let (directory, root) = root();
        let first = root.lock().unwrap();
        let other = Root::open(directory.path()).unwrap();
        let (sender, receiver) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            let second = other.lock().unwrap();
            sender.send(()).unwrap();
            drop(second);
        });
        assert!(
            receiver
                .recv_timeout(std::time::Duration::from_millis(100))
                .is_err()
        );
        drop(first);
        receiver
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        worker.join().unwrap();
    }
}
