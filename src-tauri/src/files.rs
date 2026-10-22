use crate::session::{DeviceLease, EventSink};
use crate::transfer_trace::{Edge, Stage, Trace, TraceKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, SyncSender},
    },
    thread,
    time::{Duration, Instant},
};
use takedock_files::{Entry, Identity};

pub fn quote(value: &str) -> Result<String, String> {
    if value.contains('\0') {
        return Err("invalid_argument".into());
    }
    Ok(format!("'{}'", value.replace('\'', "'\"'\"'")))
}
pub fn windows_name(name: &str) -> Result<(), String> {
    let device = name
        .split('.')
        .next()
        .unwrap_or("")
        .trim_end()
        .to_uppercase();
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.ends_with(['.', ' '])
        || name.encode_utf16().count() > 240
        || name.chars().any(|c| c < ' ' || "<>:\"/\\|?*".contains(c))
        || ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"].contains(&device.as_str())
        || ["COM", "LPT"].iter().any(|prefix| {
            device.strip_prefix(prefix).is_some_and(|suffix| {
                [
                    "1", "2", "3", "4", "5", "6", "7", "8", "9", "¹", "²", "³", "⁹",
                ]
                .contains(&suffix)
            })
        })
    {
        return Err("invalid_windows_name".into());
    }
    Ok(())
}
pub fn renamed(name: &str, stem: &str) -> Result<String, String> {
    windows_name(stem)?;
    let extension = name.rsplit_once('.').ok_or("missing_video_extension")?.1;
    let name = format!("{stem}.{extension}");
    windows_name(&name)?;
    Ok(name)
}
pub fn parse_listing(bytes: &[u8]) -> Result<Vec<Video>, String> {
    if bytes.is_empty() {
        return Ok(vec![]);
    }
    if bytes.last() != Some(&0) {
        return Err("truncated_listing".into());
    }
    let pieces = bytes[..bytes.len() - 1]
        .split(|byte| *byte == 0)
        .collect::<Vec<_>>();
    if pieces.len() % 2 != 0 {
        return Err("invalid_listing".into());
    }
    let mut videos = Vec::new();
    for pair in pieces.as_chunks::<2>().0 {
        let name = std::str::from_utf8(pair[0]).map_err(err)?;
        takedock_files::validate_name(name)?;
        let entry: Entry = serde_json::from_slice(pair[1]).map_err(err)?;
        if entry.name != name {
            return Err("invalid_listing".into());
        }
        if !name.rsplit('.').next().is_some_and(|extension| {
            ["mp4", "3gp", "m4v", "mov", "webm", "mkv"].contains(&extension.to_lowercase().as_str())
        }) {
            continue;
        }
        let modified_ns = entry.identity.modified_ns;
        videos.push((modified_ns, video_from_entry(entry)?));
    }
    videos.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.name.cmp(&b.1.name)));
    Ok(videos.into_iter().map(|(_, video)| video).collect())
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Video {
    pub name: String,
    pub size: u64,
    pub modified_ms: u64,
    pub ready: bool,
    #[serde(default)]
    pub duration_ms: Option<u64>,
    pub identity: String,
}
pub fn confirmed_rename_video(
    source: &Video,
    destination_name: &str,
    entry: Entry,
) -> Result<Video, String> {
    takedock_files::validate_name(destination_name)?;
    let original: Identity = serde_json::from_str(&source.identity).map_err(err)?;
    if entry.name != destination_name
        || entry.identity.device != original.device
        || entry.identity.inode != original.inode
    {
        return Err("source_changed".into());
    }
    video_from_entry(entry)
}
fn video_from_entry(entry: Entry) -> Result<Video, String> {
    Ok(Video {
        name: entry.name,
        size: entry.identity.size,
        modified_ms: (entry.identity.modified_ns / 1_000_000)
            .try_into()
            .map_err(err)?,
        ready: entry.ready,
        duration_ms: entry.duration_ms,
        identity: serde_json::to_string(&entry.identity).map_err(err)?,
    })
}
fn err(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobKind {
    Copy,
    Move,
    Delete,
    Rename,
}
#[derive(Clone, Debug, Serialize)]
pub struct JobEvent {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discard_operation_id: Option<String>,
    pub id: String,
    pub generation: u64,
    pub kind: JobKind,
    pub status: String,
    pub phase: String,
    pub name: String,
    pub bytes: u64,
    pub total: u64,
    pub completed: usize,
    pub count: usize,
    pub message: String,
    pub results: Vec<FileResult>,
}
#[derive(Clone, Debug, Serialize)]
pub struct FileResult {
    pub name: String,
    pub status: String,
    pub message: String,
    pub requested_name: Option<String>,
    pub destination_name: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct RenameUpdate {
    pub generation: u64,
    pub id: String,
    pub sequence: u64,
    pub source_name: String,
    pub source_identity: String,
    pub video: Video,
}
struct Job {
    event: JobEvent,
    lease: DeviceLease,
    videos: Vec<Video>,
    destination: PathBuf,
    new_stem: Option<String>,
    cancel: Arc<AtomicBool>,
    trace: Trace,
}
enum Message {
    List(DeviceLease, mpsc::Sender<Result<Vec<Video>, String>>),
    Job(Box<Job>),
    Discard(Box<crate::guarded_delete::DiscardJob>),
    Stop,
}
pub struct FileService {
    sender: SyncSender<Message>,
    jobs: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    worker: Mutex<Option<thread::JoinHandle<()>>>,
    sink: EventSink,
    trace: Trace,
}
impl FileService {
    pub fn start_discard_delete(
        &self,
        lease: DeviceLease,
        video: Video,
        operation_id: String,
        authorize: Arc<dyn Fn() -> bool + Send + Sync>,
        complete: Arc<dyn Fn(crate::discard::DeleteOutcome) + Send + Sync>,
    ) -> Result<String, String> {
        self.start_discard_delete_checked(
            lease,
            video,
            operation_id,
            Arc::new(|| true),
            authorize,
            complete,
        )
    }
    pub(crate) fn start_discard_delete_checked(
        &self,
        lease: DeviceLease,
        video: Video,
        operation_id: String,
        should_prepare: Arc<dyn Fn() -> bool + Send + Sync>,
        authorize: Arc<dyn Fn() -> bool + Send + Sync>,
        complete: Arc<dyn Fn(crate::discard::DeleteOutcome) + Send + Sync>,
    ) -> Result<String, String> {
        takedock_files::validate_name(&video.name)?;
        let identity: Identity = serde_json::from_str(&video.identity).map_err(err)?;
        if !video.ready || identity.size != video.size {
            return Err("video_not_finalized".into());
        }
        if operation_id.is_empty()
            || operation_id.len() > 128
            || lease.cancelled.load(Ordering::Acquire)
        {
            return Err("discard_cancelled".into());
        }
        let id = uuid::Uuid::new_v4().to_string();
        let cancel = Arc::new(AtomicBool::new(false));
        let event = JobEvent {
            discard_operation_id: Some(operation_id.clone()),
            id: id.clone(),
            generation: lease.generation,
            kind: JobKind::Delete,
            status: "queued".into(),
            phase: "preparing".into(),
            name: video.name.clone(),
            bytes: 0,
            total: video.size,
            completed: 0,
            count: 1,
            message: String::new(),
            results: Vec::new(),
        };
        self.jobs.lock().unwrap().insert(id.clone(), cancel.clone());
        let job = crate::guarded_delete::DiscardJob {
            should_prepare,
            event: event.clone(),
            lease,
            video,
            operation_id,
            cancel,
            authorize,
            complete,
        };
        if self
            .sender
            .try_send(Message::Discard(Box::new(job)))
            .is_err()
        {
            self.jobs.lock().unwrap().remove(&id);
            return Err("file_queue_full".into());
        }
        emit(&self.sink, &event);
        Ok(id)
    }
    pub fn start(resources: PathBuf, sink: EventSink) -> Self {
        let (sender, receiver) = mpsc::sync_channel(16);
        let jobs = Arc::new(Mutex::new(HashMap::new()));
        let active = jobs.clone();
        let worker_sink = sink.clone();
        let worker = thread::spawn(move || {
            let sink = worker_sink;
            let mut remote = Remote::new(resources);
            let mut sequence = 0_u64;
            for message in receiver {
                match message {
                    Message::Discard(job) => {
                        let id = job.event.id.clone();
                        crate::guarded_delete::run(&mut remote, *job, &sink);
                        active.lock().unwrap().remove(&id);
                    }
                    Message::List(lease, reply) => {
                        let result = remote
                            .prepare(&lease)
                            .and_then(|_| remote.output(&lease, &["list"], 60_000))
                            .and_then(|bytes| parse_listing(&bytes));
                        let _ = reply.send(result);
                    }
                    Message::Job(mut job) => {
                        sequence += 1;
                        let key = TraceKey::new(&job.event.id, job.lease.generation, None);
                        job.trace.mark(&key, Stage::Worker, Edge::Mark);
                        job.event.status = "running".into();
                        emit(&sink, &job.event);
                        let result = job
                            .trace
                            .measure(&key, Stage::Prepare, || remote.prepare(&job.lease))
                            .and_then(|_| execute(&remote, &mut job, &sink, sequence));
                        job.event.status = if result.is_ok() {
                            "completed"
                        } else if job.cancel.load(Ordering::Acquire) {
                            "cancelled"
                        } else {
                            "failed"
                        }
                        .into();
                        job.event.message = result.err().unwrap_or_default();
                        for video in job.videos.iter().skip(job.event.results.len()) {
                            job.event.results.push(FileResult {
                                name: video.name.clone(),
                                status: "skipped".into(),
                                message: job.event.message.clone(),
                                requested_name: None,
                                destination_name: None,
                            });
                        }
                        job.trace.mark(&key, Stage::Terminal, Edge::Mark);
                        emit(&sink, &job.event);
                        job.trace.complete_job(&key);
                        active.lock().unwrap().remove(&job.event.id);
                    }
                    Message::Stop => break,
                }
            }
        });
        Self {
            sender,
            jobs,
            worker: Mutex::new(Some(worker)),
            sink,
            trace: Trace::from_env(),
        }
    }
    pub fn list(&self, lease: DeviceLease) -> Result<Vec<Video>, String> {
        let (send, receive) = mpsc::channel();
        self.sender
            .try_send(Message::List(lease, send))
            .map_err(|_| "file_queue_full")?;
        receive.recv().map_err(err)?
    }
    pub fn start_job(
        &self,
        lease: DeviceLease,
        kind: JobKind,
        videos: Vec<Video>,
        destination: PathBuf,
        new_stem: Option<String>,
    ) -> Result<String, String> {
        let accepted_at = self.trace.stamp();
        if videos.is_empty() || videos.len() > 10_000 {
            return Err("invalid_selection".into());
        }
        if kind == JobKind::Rename && (videos.len() != 1 || new_stem.is_none()) {
            return Err("rename_requires_one_video".into());
        }
        let mut names = std::collections::HashSet::new();
        for video in &videos {
            takedock_files::validate_name(&video.name)?;
            let identity: Identity = serde_json::from_str(&video.identity).map_err(err)?;
            if identity.size != video.size || !video.ready {
                return Err("video_not_finalized".into());
            }
            if !names.insert(&video.name) {
                return Err("duplicate_selection".into());
            }
            if matches!(kind, JobKind::Copy | JobKind::Move) {
                windows_name(&video.name)?;
            }
        }
        if kind == JobKind::Rename {
            renamed(&videos[0].name, new_stem.as_deref().unwrap())?;
        }
        if matches!(kind, JobKind::Copy | JobKind::Move)
            && (!destination.is_absolute() || !destination.is_dir())
        {
            return Err("destination_required".into());
        }
        if lease.cancelled.load(Ordering::Acquire) {
            return Err("session_replaced".into());
        }
        let id = uuid::Uuid::new_v4().to_string();
        let key = TraceKey::new(&id, lease.generation, None);
        self.trace
            .record_at(&key, Stage::Accepted, Edge::Mark, accepted_at);
        let cancel = Arc::new(AtomicBool::new(false));
        let event = JobEvent {
            discard_operation_id: None,
            id: id.clone(),
            generation: lease.generation,
            kind,
            status: "queued".into(),
            phase: "preparing".into(),
            name: String::new(),
            bytes: 0,
            total: videos
                .iter()
                .try_fold(0u64, |sum, v| sum.checked_add(v.size))
                .ok_or("selection_too_large")?,
            completed: 0,
            count: videos.len(),
            message: String::new(),
            results: Vec::new(),
        };
        self.jobs.lock().unwrap().insert(id.clone(), cancel.clone());
        let queued = event.clone();
        let enqueued_at = self.trace.stamp();
        if self
            .sender
            .try_send(Message::Job(Box::new(Job {
                event,
                lease,
                videos,
                destination,
                new_stem,
                cancel,
                trace: self.trace.clone(),
            })))
            .is_err()
        {
            self.jobs.lock().unwrap().remove(&id);
            return Err("file_queue_full".into());
        }
        self.trace
            .record_at(&key, Stage::Enqueued, Edge::Mark, enqueued_at);
        emit(&self.sink, &queued);
        Ok(id)
    }
    pub fn cancel(&self, id: &str) -> Result<(), String> {
        self.jobs
            .lock()
            .unwrap()
            .get(id)
            .ok_or("job_not_found")?
            .store(true, Ordering::Release);
        Ok(())
    }
    pub fn shutdown(&self) {
        for flag in self.jobs.lock().unwrap().values() {
            flag.store(true, Ordering::Release)
        }
        let mut worker = self.worker.lock().unwrap();
        if let Some(handle) = worker.take() {
            let _ = self.sender.send(Message::Stop);
            let _ = handle.join();
        }
    }
}
impl Drop for FileService {
    fn drop(&mut self) {
        self.shutdown()
    }
}
fn emit(sink: &EventSink, event: &JobEvent) {
    sink("file-job", serde_json::to_value(event).unwrap())
}
type Remote = crate::helper::HelperClient;
impl Remote {
    fn job_output(&self, job: &Job, args: &[&str], timeout: u64) -> Result<Vec<u8>, String> {
        check(job)?;
        // Mutation acknowledgement requires shell-v2's remote exit status.
        // Legacy exec-out can exit 0 even when the helper rejected the operation.
        job.lease.adb.output_with_cancellation(
            &job.lease.serial,
            &["shell", "-T", &self.command(args)?],
            timeout,
            &[&job.lease.cancelled, &job.cancel],
        )
    }
}
fn check(job: &Job) -> Result<(), String> {
    if job.cancel.load(Ordering::Acquire) {
        Err("operation_cancelled".into())
    } else if job.lease.cancelled.load(Ordering::Acquire) {
        Err("session_replaced".into())
    } else {
        Ok(())
    }
}
pub(crate) fn hash_path(path: &Path) -> Result<String, String> {
    let mut file = std::fs::File::open(path).map_err(err)?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 256 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(err)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(hash
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}
fn execute(remote: &Remote, job: &mut Job, sink: &EventSink, sequence: u64) -> Result<(), String> {
    let mut failure = None;
    for (file_index, video) in job.videos.clone().into_iter().enumerate() {
        let key = TraceKey::new(&job.event.id, job.lease.generation, Some(file_index));
        if check(job).is_err() {
            job.event.results.push(FileResult {
                name: video.name.clone(),
                status: "cancelled".into(),
                message: "operation_cancelled".into(),
                requested_name: None,
                destination_name: None,
            });
            return Err("operation_cancelled".into());
        }
        let requested_name = if job.event.kind == JobKind::Rename {
            Some(renamed(&video.name, job.new_stem.as_deref().unwrap())?)
        } else {
            None
        };
        let mut destination_name = None;
        let result = (|| -> Result<(), String> {
            check(job)?;
            job.event.name = video.name.clone();
            job.event.bytes = 0;
            job.event.total = video.size;
            job.event.phase = match job.event.kind {
                JobKind::Copy | JobKind::Move => "copying",
                JobKind::Delete => "deleting",
                JobKind::Rename => "renaming",
            }
            .into();
            emit(sink, &job.event);
            job.trace
                .measure(&key, Stage::InitialStat, || remote.stat(&job.lease, &video))?;
            match job.event.kind {
                JobKind::Copy | JobKind::Move => {
                    let destination = job.destination.join(&video.name);
                    if destination.exists() {
                        return Err("destination_exists".into());
                    }
                    let partial = tempfile::Builder::new()
                        .prefix(".takedock-")
                        .suffix(".partial")
                        .tempfile_in(&job.destination)
                        .map_err(err)?;
                    job.trace.measure(&key, Stage::Stream, || {
                        stream(
                            remote,
                            job,
                            &video,
                            partial.as_file().try_clone().map_err(err)?,
                            sink,
                        )
                    })?;
                    check(job)?;
                    job.event.bytes = video.size;
                    job.event.phase = "verifying".into();
                    emit(sink, &job.event);
                    job.trace
                        .measure(&key, Stage::Sync, || partial.as_file().sync_all())
                        .map_err(err)?;
                    let stored_size = job
                        .trace
                        .measure(&key, Stage::StoredSize, || partial.as_file().metadata())
                        .map_err(err)?
                        .len();
                    if stored_size != video.size {
                        return Err("copy_size_mismatch".into());
                    }
                    job.trace.measure(&key, Stage::PostTransferStat, || {
                        remote.stat(&job.lease, &video)
                    })?;
                    let verification_span = job.trace.span(&key, Stage::Verify);
                    let expected =
                        String::from_utf8(job.trace.measure(&key, Stage::RemoteHash, || {
                            remote.job_output(job, &["hash", &video.name, &video.identity], 600_000)
                        })?)
                        .map_err(err)?;
                    if expected.trim()
                        != job
                            .trace
                            .measure(&key, Stage::LocalHash, || hash_path(partial.path()))?
                    {
                        return Err("copy_hash_mismatch".into());
                    }
                    drop(verification_span);
                    check(job)?;
                    job.trace
                        .measure(&key, Stage::Publish, || {
                            partial.persist_noclobber(&destination)
                        })
                        .map_err(err)?;
                    if job.event.kind == JobKind::Move {
                        job.event.phase = "deleting".into();
                        emit(sink, &job.event);
                        check(job)?;
                        job.trace
                            .measure(&key, Stage::MoveStat, || remote.stat(&job.lease, &video))?;
                        job.trace.measure(&key, Stage::Delete, || {
                            remote.job_output(
                                job,
                                &["delete", &video.name, &video.identity],
                                30_000,
                            )
                        })?;
                    }
                }
                JobKind::Delete => {
                    check(job)?;
                    remote.job_output(job, &["delete", &video.name, &video.identity], 30_000)?;
                }
                JobKind::Rename => {
                    let new_name = requested_name.as_deref().unwrap();
                    check(job)?;
                    remote.job_output(
                        job,
                        &["rename", &video.name, new_name, &video.identity],
                        30_000,
                    )?;
                    destination_name = Some(new_name.to_owned());
                    let confirmed = remote
                        .output(&job.lease, &["inspect", new_name], 5_000)
                        .and_then(|bytes| serde_json::from_slice::<Entry>(&bytes).map_err(err))
                        .and_then(|entry| confirmed_rename_video(&video, new_name, entry));
                    match confirmed {
                        Ok(renamed_video) => (sink)(
                            "file-renamed",
                            serde_json::to_value(RenameUpdate {
                                generation: job.lease.generation,
                                id: job.event.id.clone(),
                                sequence,
                                source_name: video.name.clone(),
                                source_identity: video.identity.clone(),
                                video: renamed_video,
                            })
                            .expect("rename update serialization"),
                        ),
                        Err(_) => (sink)(
                            "file-metadata-warning",
                            serde_json::json!({
                                "generation":job.lease.generation,"id":job.event.id,"code":"rename_refresh_pending"
                            }),
                        ),
                    }
                }
            }
            if job.event.kind != JobKind::Copy {
                let _index_span = job.trace.span(&key, Stage::Index);
                let mut names = vec![video.name.clone()];
                if job.event.kind == JobKind::Rename {
                    names.push(renamed(&video.name, job.new_stem.as_deref().unwrap())?);
                }
                for name in names {
                    let encoded = name
                        .as_bytes()
                        .iter()
                        .map(|b| {
                            if b.is_ascii_alphanumeric() || b"-._~".contains(b) {
                                (*b as char).to_string()
                            } else {
                                format!("%{b:02X}")
                            }
                        })
                        .collect::<String>();
                    let uri = format!("file:///sdcard/DCIM/OpenCamera/{encoded}");
                    if job
                        .lease
                        .output(
                            &[
                                "shell",
                                "am",
                                "broadcast",
                                "-a",
                                "android.intent.action.MEDIA_SCANNER_SCAN_FILE",
                                "-d",
                                &uri,
                            ],
                            5000,
                        )
                        .is_err()
                    {
                        (sink)(
                            "file-index-warning",
                            serde_json::json!({"generation":job.lease.generation,"id":job.event.id,"code":"media_index_pending"}),
                        );
                    }
                }
            }
            Ok(())
        })();
        let status = if result.is_ok() {
            job.event.completed += 1;
            "completed"
        } else if job.cancel.load(Ordering::Acquire) {
            "cancelled"
        } else {
            "failed"
        };
        let message = result.err().unwrap_or_default();
        if !message.is_empty() {
            failure = Some(message.clone());
        }
        job.event.results.push(FileResult {
            name: video.name.clone(),
            status: status.into(),
            message,
            requested_name,
            destination_name,
        });
        emit(sink, &job.event);
    }
    match failure {
        Some(message) => Err(message),
        None => Ok(()),
    }
}
fn stream(
    remote: &Remote,
    job: &Job,
    video: &Video,
    mut file: std::fs::File,
    sink: &EventSink,
) -> Result<(), String> {
    let command = remote.command(&["read", &video.name, &video.identity])?;
    let mut child = job
        .lease
        .adb
        .spawn(Some(&job.lease.serial), &["exec-out", &command], false)?;
    let mut stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let errors = thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stderr.take(64 * 1024).read_to_end(&mut bytes);
        bytes
    });
    let progress = Arc::new(AtomicU64::new(0));
    let written = progress.clone();
    let maximum = video.size;
    let read_failed = Arc::new(AtomicBool::new(false));
    let failure_flag = read_failed.clone();
    let mut event = job.event.clone();
    let sink = sink.clone();
    let output = thread::spawn(move || {
        let result = (|| -> Result<(), String> {
            let mut buffer = [0; 256 * 1024];
            let mut last = Instant::now();
            let mut count = 0u64;
            loop {
                let read = stdout.read(&mut buffer).map_err(err)?;
                if read == 0 {
                    break;
                }
                count = count.checked_add(read as u64).ok_or("copy_size_mismatch")?;
                if count > maximum {
                    return Err("copy_size_mismatch".into());
                }
                file.write_all(&buffer[..read]).map_err(err)?;
                written.store(count, Ordering::Release);
                if last.elapsed() > Duration::from_millis(100) {
                    event.bytes = count;
                    emit(&sink, &event);
                    last = Instant::now();
                }
            }
            if count != maximum {
                return Err("copy_size_mismatch".into());
            }
            Ok(())
        })();
        failure_flag.store(result.is_err(), Ordering::Release);
        result
    });
    let mut last = Instant::now();
    let mut previous = 0;
    let mut failure = None;
    let status = loop {
        if let Err(error) = check(job) {
            failure = Some(error);
            let _ = child.kill();
            break child.wait().map_err(err)?;
        }
        let current = progress.load(Ordering::Acquire);
        if current != previous {
            last = Instant::now();
            previous = current;
        }
        if last.elapsed() > Duration::from_secs(180) || read_failed.load(Ordering::Acquire) {
            failure = Some("transfer_interrupted".into());
            let _ = child.kill();
            break child.wait().map_err(err)?;
        }
        if let Some(status) = child.try_wait().map_err(err)? {
            break status;
        }
        thread::sleep(Duration::from_millis(10));
    };
    let result = output.join().map_err(|_| "copy_worker_panicked")?;
    let detail = String::from_utf8_lossy(&errors.join().map_err(|_| "copy_error_worker_panicked")?)
        .trim()
        .to_owned();
    if let Some(error) = failure {
        return Err(error);
    }
    if !status.success() {
        return Err(if detail.is_empty() {
            "transfer_failed".into()
        } else {
            detail
        });
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rename_fixture() -> (Video, Entry) {
        let identity = Identity {
            device: 9_007_199_254_740_993,
            inode: 9_007_199_254_740_995,
            size: 7,
            modified_ns: 1_700_000_000_123_456_789,
            changed_ns: 1_700_000_000_123_456_790,
        };
        let source = Video {
            name: "old.MP4".into(),
            size: 7,
            modified_ms: 1_700_000_000_123,
            ready: true,
            duration_ms: Some(100),
            identity: serde_json::to_string(&identity).unwrap(),
        };
        let mut destination = Entry {
            name: "رحلة'a.MP4".into(),
            identity,
            ready: true,
            duration_ms: Some(300),
        };
        destination.identity.changed_ns += 1;
        (source, destination)
    }
    #[test]
    fn confirmed_rename_requires_expected_name_and_inode() {
        let (source, entry) = rename_fixture();
        assert!(confirmed_rename_video(&source, "رحلة'a.MP4", entry.clone()).is_ok());
        for mismatch in ["name", "device", "inode"] {
            let mut wrong = entry.clone();
            match mismatch {
                "name" => wrong.name = "different.MP4".into(),
                "device" => wrong.identity.device += 1,
                _ => wrong.identity.inode += 1,
            }
            assert!(confirmed_rename_video(&source, "رحلة'a.MP4", wrong).is_err());
        }
        assert!(confirmed_rename_video(&source, "../escape.MP4", entry).is_err());
    }
    #[test]
    fn confirmed_rename_preserves_exact_large_identity() {
        let (source, entry) = rename_fixture();
        let video = confirmed_rename_video(&source, "رحلة'a.MP4", entry).unwrap();
        assert_eq!(video.name, "رحلة'a.MP4");
        assert_eq!(video.size, 7);
        assert_eq!(video.modified_ms, 1_700_000_000_123);
        assert_eq!(video.duration_ms, Some(300));
        assert!(video.ready);
        assert_ne!(video.identity, source.identity);
        assert!(video.identity.contains("\"device\":9007199254740993"));
        assert!(video.identity.contains("\"inode\":9007199254740995"));
        assert!(
            video
                .identity
                .contains("\"changed_ns\":1700000000123456791")
        );
    }
    #[test]
    fn remote_arguments_quote_apostrophes_unicode_and_newlines() {
        assert_eq!(
            quote("a'b\nالعربية.mp4").unwrap(),
            "'a'\"'\"'b\nالعربية.mp4'"
        );
        assert!(quote("a\0b").is_err());
    }
    #[test]
    fn windows_names_reject_paths_device_names_and_ambiguous_suffixes() {
        for name in [
            "",
            "..",
            "../a.mp4",
            "a\\b.mp4",
            "NUL.mp4",
            "com1.mp4",
            "LPT⁹.mp4",
            "a.mp4.",
            "a.mp4 ",
            "a:b.mp4",
            "a\nb.mp4",
        ] {
            assert!(windows_name(name).is_err(), "{name}");
        }
        for name in ["hello.mp4", "رحلة ١.mp4", "a'b.mp4", "a&b.mp4"] {
            assert!(windows_name(name).is_ok(), "{name}");
        }
    }
    #[test]
    fn rename_preserves_video_extension_and_rejects_path_stems() {
        assert_eq!(renamed("old.MP4", "رحلة").unwrap(), "رحلة.MP4");
        assert!(renamed("old.mp4", "../escape").is_err());
    }
    #[test]
    fn binary_listing_preserves_names_and_rejects_truncation() {
        let entry = Entry {
            name: "a'b.mp4".into(),
            identity: Identity {
                device: 1,
                inode: 2,
                size: 7,
                modified_ns: 1_700_000_000_123_456_789,
                changed_ns: 1_700_000_000_123_456_790,
            },
            ready: true,
            duration_ms: Some(100),
        };
        let mut bytes = entry.name.as_bytes().to_vec();
        bytes.push(0);
        bytes.extend(serde_json::to_vec(&entry).unwrap());
        bytes.push(0);
        let videos = parse_listing(&bytes).unwrap();
        assert_eq!(videos[0].name, entry.name);
        let decoded: Identity = serde_json::from_str(&videos[0].identity).unwrap();
        assert_eq!(decoded, entry.identity);
        bytes.pop();
        assert!(parse_listing(&bytes).is_err());
    }
}
#[test]
fn listing_is_newest_first_and_carries_exact_duration() {
    let mut bytes = Vec::new();
    for (name, modified, duration) in [
        ("old.mp4", 100_000_001u128, Some(123_514u64)),
        ("new.mp4", 100_000_099, Some(500)),
    ] {
        bytes.extend(name.as_bytes());
        bytes.push(0);
        bytes.extend(serde_json::to_vec(&serde_json::json!({"name":name,"identity":{"device":1,"inode":1,"size":20,"modified_ns":modified,"changed_ns":modified},"ready":true,"duration_ms":duration})).unwrap());
        bytes.push(0);
    }
    let videos = parse_listing(&bytes).unwrap();
    assert_eq!(videos[0].name, "new.mp4");
    assert_eq!(videos[1].duration_ms, Some(123_514));
}
