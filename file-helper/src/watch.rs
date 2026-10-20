use crate::{Identity, Root, validate_name};
use serde::{Deserialize, Serialize};
use std::io::Write;

pub const MAX_FRAME: usize = 64 * 1024;
pub const MAX_ENTRIES: usize = 10_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestEntry {
    pub name: String,
    #[serde(with = "opaque_identity")]
    pub identity: Identity,
}
mod opaque_identity {
    use super::Identity;
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(identity: &Identity, serializer: S) -> Result<S::Ok, S::Error> {
        let text = serde_json::to_string(identity).map_err(serde::ser::Error::custom)?;
        serializer.serialize_str(&text)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Identity, D::Error> {
        let text = String::deserialize(deserializer)?;
        serde_json::from_str(&text).map_err(serde::de::Error::custom)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum WatchBody {
    BaselineBegin,
    BaselineEntry { entry: ManifestEntry },
    BaselineEnd,
    Created { entry: ManifestEntry },
    Changed { entry: ManifestEntry },
    Closed { entry: ManifestEntry },
    Renamed { from: String, entry: ManifestEntry },
    Removed { name: String },
    Lost { code: String },
    Heartbeat,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WatchFrame {
    pub version: u32,
    pub stream: String,
    pub sequence: u64,
    pub body: WatchBody,
}

pub fn decode_frame(line: &[u8]) -> Result<WatchFrame, String> {
    if line.len() > MAX_FRAME {
        return Err("file_watch_frame_too_large".into());
    }
    let frame: WatchFrame = serde_json::from_slice(line).map_err(|_| "file_watch_protocol")?;
    if frame.version != 1
        || frame.sequence == 0
        || frame.stream.is_empty()
        || frame.stream.len() > 128
    {
        return Err("file_watch_protocol".into());
    }
    match &frame.body {
        WatchBody::BaselineEntry { entry }
        | WatchBody::Created { entry }
        | WatchBody::Changed { entry }
        | WatchBody::Closed { entry } => validate_name(&entry.name)?,
        WatchBody::Renamed { from, entry } => {
            validate_name(from)?;
            validate_name(&entry.name)?;
            if from == &entry.name {
                return Err("file_watch_protocol".into());
            }
        }
        WatchBody::Removed { name } => validate_name(name)?,
        _ => {}
    }
    Ok(frame)
}

pub fn run(root: &Root, stream: &str, output: &mut impl Write) -> Result<(), String> {
    if stream.is_empty() || stream.len() > 128 {
        return Err("file_watch_protocol".into());
    }
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        native::run(root, stream, output)
    }
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        let _ = (root, output);
        Err("file_watch_unsupported".into())
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
mod native {
    use super::*;
    use std::{
        collections::BTreeMap,
        ffi::CString,
        io,
        os::fd::{AsRawFd, FromRawFd, OwnedFd},
        time::{Duration, Instant},
    };

    struct Watch {
        fd: OwnedFd,
    }
    struct Event {
        mask: u32,
        cookie: u32,
        name: Option<String>,
    }
    impl Watch {
        fn open(root: &Root) -> Result<Self, String> {
            let raw = unsafe { libc::inotify_init1(libc::IN_CLOEXEC | libc::IN_NONBLOCK) };
            if raw < 0 {
                return Err(io::Error::last_os_error().to_string());
            }
            let fd = unsafe { OwnedFd::from_raw_fd(raw) };
            let path =
                CString::new(format!("/proc/self/fd/{}", root.directory.as_raw_fd())).unwrap();
            let mask = libc::IN_CREATE
                | libc::IN_MODIFY
                | libc::IN_CLOSE_WRITE
                | libc::IN_ATTRIB
                | libc::IN_DELETE
                | libc::IN_MOVED_FROM
                | libc::IN_MOVED_TO
                | libc::IN_DELETE_SELF
                | libc::IN_MOVE_SELF
                | libc::IN_UNMOUNT
                | libc::IN_ONLYDIR;
            if unsafe { libc::inotify_add_watch(fd.as_raw_fd(), path.as_ptr(), mask) } < 0 {
                return Err(io::Error::last_os_error().to_string());
            }
            Ok(Self { fd })
        }
        fn drain(&self) -> Result<Vec<Event>, String> {
            let mut events = Vec::new();
            let mut buffer = [0u8; MAX_FRAME];
            loop {
                let count = unsafe {
                    libc::read(
                        self.fd.as_raw_fd(),
                        buffer.as_mut_ptr().cast(),
                        buffer.len(),
                    )
                };
                if count < 0 {
                    let error = io::Error::last_os_error();
                    if error.kind() == io::ErrorKind::WouldBlock {
                        return Ok(events);
                    }
                    if error.kind() == io::ErrorKind::Interrupted {
                        continue;
                    }
                    return Err(error.to_string());
                }
                if count == 0 {
                    return Err("file_watch_closed".into());
                }
                let mut at = 0;
                while at < count as usize {
                    let header = std::mem::size_of::<libc::inotify_event>();
                    if count as usize - at < header {
                        return Err("file_watch_protocol".into());
                    }
                    let event = unsafe {
                        std::ptr::read_unaligned(
                            buffer.as_ptr().add(at).cast::<libc::inotify_event>(),
                        )
                    };
                    let end = at
                        .checked_add(header)
                        .and_then(|n| n.checked_add(event.len as usize))
                        .ok_or("file_watch_protocol")?;
                    if end > count as usize {
                        return Err("file_watch_protocol".into());
                    }
                    let bytes = &buffer[at + header..end];
                    let name = if bytes.is_empty() {
                        None
                    } else {
                        let zero = bytes
                            .iter()
                            .position(|b| *b == 0)
                            .ok_or("filename_encoding")?;
                        Some(
                            std::str::from_utf8(&bytes[..zero])
                                .map_err(|_| "filename_encoding")?
                                .to_string(),
                        )
                    };
                    if !name
                        .as_ref()
                        .is_some_and(|name| name.starts_with(".takedock-"))
                    {
                        events.push(Event {
                            mask: event.mask,
                            cookie: event.cookie,
                            name,
                        });
                    }
                    at = end;
                    if events.len() > MAX_ENTRIES {
                        return Err("file_watch_overflow".into());
                    }
                }
            }
        }
        fn wait(&self, timeout: Duration) -> Result<(), String> {
            let mut fd = libc::pollfd {
                fd: self.fd.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            let status = unsafe { libc::poll(&mut fd, 1, timeout.as_millis().min(1000) as i32) };
            if status < 0 && io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
                return Err(io::Error::last_os_error().to_string());
            }
            if fd.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0 {
                return Err("file_watch_closed".into());
            }
            Ok(())
        }
    }
    // A name can already have changed by the time a queued close/modify is read.
    // Resolve only an explicit unique in-batch kernel cookie pair, never a scan.
    fn paired_destination<'a>(events: &'a [Event], from: &str) -> Option<&'a str> {
        let mut sources = events
            .iter()
            .filter(|e| e.mask & libc::IN_MOVED_FROM != 0 && e.name.as_deref() == Some(from));
        let source = sources.next()?;
        if source.cookie == 0 || sources.next().is_some() {
            return None;
        }
        let mut targets = events
            .iter()
            .filter(|e| e.mask & libc::IN_MOVED_TO != 0 && e.cookie == source.cookie);
        let target = targets.next()?.name.as_deref()?;
        if targets.next().is_some() || target == from {
            return None;
        }
        Some(target)
    }
    fn emit(
        output: &mut impl Write,
        stream: &str,
        sequence: &mut u64,
        body: WatchBody,
    ) -> Result<(), String> {
        *sequence += 1;
        let frame = WatchFrame {
            version: 1,
            stream: stream.into(),
            sequence: *sequence,
            body,
        };
        serde_json::to_writer(&mut *output, &frame).map_err(|e| e.to_string())?;
        output
            .write_all(b"\n")
            .and_then(|_| output.flush())
            .map_err(|e| e.to_string())
    }
    pub(super) fn run(root: &Root, stream: &str, output: &mut impl Write) -> Result<(), String> {
        let watcher = Watch::open(root)?;
        let mut sequence = 0;
        let mut complete = false;
        let mut known = BTreeMap::new();
        for _ in 0..3 {
            known.clear();
            emit(output, stream, &mut sequence, WatchBody::BaselineBegin)?;
            for entry in root.manifest()? {
                known.insert(entry.name.clone(), entry.identity.clone());
                emit(
                    output,
                    stream,
                    &mut sequence,
                    WatchBody::BaselineEntry { entry },
                )?;
            }
            if !watcher.drain()?.is_empty() {
                continue;
            }
            emit(output, stream, &mut sequence, WatchBody::BaselineEnd)?;
            complete = true;
            break;
        }
        if !complete {
            emit(
                output,
                stream,
                &mut sequence,
                WatchBody::Lost {
                    code: "file_watch_unstable_baseline".into(),
                },
            )?;
            return Err("file_watch_unstable_baseline".into());
        }
        let mut heartbeat = Instant::now();
        let mut pending: Option<(u32, String, crate::Identity, Instant)> = None;
        loop {
            let mut wait = Duration::from_millis(1000).saturating_sub(heartbeat.elapsed());
            if let Some((_, _, _, since)) = &pending {
                wait = wait.min(Duration::from_millis(250).saturating_sub(since.elapsed()));
            }
            watcher.wait(wait)?;
            let events = watcher.drain()?;
            for event in &events {
                let bad = libc::IN_Q_OVERFLOW
                    | libc::IN_IGNORED
                    | libc::IN_UNMOUNT
                    | libc::IN_DELETE_SELF
                    | libc::IN_MOVE_SELF
                    | libc::IN_ISDIR;
                let lost = || WatchBody::Lost {
                    code: "file_watch_history_lost".into(),
                };
                let body = if event.mask & bad != 0 {
                    WatchBody::Lost {
                        code: "file_watch_history_lost".into(),
                    }
                } else if let Some(name) = &event.name {
                    validate_name(name)?;
                    if event.mask & libc::IN_MOVED_TO != 0 {
                        if let Some((cookie, from, previous, _)) = pending.take()
                            && event.cookie == cookie
                            && cookie != 0
                            && !known.contains_key(name)
                        {
                            let identity = root.stat(name)?;
                            if identity.device == previous.device
                                && identity.inode == previous.inode
                            {
                                known.insert(name.clone(), identity.clone());
                                emit(
                                    output,
                                    stream,
                                    &mut sequence,
                                    WatchBody::Renamed {
                                        from,
                                        entry: ManifestEntry {
                                            name: name.clone(),
                                            identity,
                                        },
                                    },
                                )?;
                                continue;
                            }
                        }
                        lost()
                    } else if pending.is_some() {
                        lost()
                    } else if event.mask & libc::IN_MOVED_FROM != 0 {
                        if let Some(previous) = known.remove(name).filter(|_| event.cookie != 0) {
                            pending = Some((event.cookie, name.clone(), previous, Instant::now()));
                            continue;
                        }
                        lost()
                    } else {
                        if event.mask & libc::IN_DELETE != 0 {
                            known.remove(name);
                            WatchBody::Removed { name: name.clone() }
                        } else {
                            let identity = root.stat(name).or_else(|error| {
                                paired_destination(&events, name)
                                    .ok_or(error)
                                    .and_then(|target| root.stat(target))
                            })?;
                            let entry = ManifestEntry {
                                identity,
                                name: name.clone(),
                            };
                            if known.get(name).is_some_and(|old| {
                                old.device != entry.identity.device
                                    || old.inode != entry.identity.inode
                            }) {
                                emit(output, stream, &mut sequence, lost())?;
                                return Err("file_watch_history_lost".into());
                            }
                            known.insert(name.clone(), entry.identity.clone());
                            if event.mask & libc::IN_CREATE != 0 {
                                WatchBody::Created { entry }
                            } else if event.mask & libc::IN_CLOSE_WRITE != 0 {
                                WatchBody::Closed { entry }
                            } else if event.mask & (libc::IN_MODIFY | libc::IN_ATTRIB) != 0 {
                                WatchBody::Changed { entry }
                            } else {
                                WatchBody::Lost {
                                    code: "file_watch_unknown_event".into(),
                                }
                            }
                        }
                    }
                } else {
                    WatchBody::Lost {
                        code: "file_watch_unknown_event".into(),
                    }
                };
                let lost = matches!(body, WatchBody::Lost { .. });
                emit(output, stream, &mut sequence, body)?;
                if lost {
                    return Err("file_watch_history_lost".into());
                }
            }
            if pending
                .as_ref()
                .is_some_and(|(_, _, _, since)| since.elapsed() >= Duration::from_millis(250))
            {
                emit(
                    output,
                    stream,
                    &mut sequence,
                    WatchBody::Lost {
                        code: "file_watch_unmatched_move".into(),
                    },
                )?;
                return Err("file_watch_unmatched_move".into());
            }
            if heartbeat.elapsed() >= Duration::from_millis(1000) {
                emit(output, stream, &mut sequence, WatchBody::Heartbeat)?;
                heartbeat = Instant::now();
            }
        }
    }
}
