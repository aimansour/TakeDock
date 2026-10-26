use crate::session::{DeviceLease, EventSink};
use crate::transfer_trace::{Edge, Stage, Trace, TraceKey};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, SyncSender},
    },
    thread,
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
    Mark,
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
#[derive(Clone, Debug, Serialize)]
pub struct HandoffUpdate {
    pub generation: u64,
    pub job_id: String,
    pub operation_id: String,
    pub sequence: u64,
    pub source_name: String,
    pub source_identity: String,
}
pub type HandoffCommit = Arc<dyn Fn(&HandoffUpdate) + Send + Sync>;
struct Job {
    event: JobEvent,
    lease: DeviceLease,
    videos: Vec<Video>,
    new_stem: Option<String>,
    cancel: Arc<AtomicBool>,
    trace: Trace,
    on_handoff: Option<HandoffCommit>,
}
#[derive(Default)]
struct Reservations {
    owners: HashMap<(u64, u64, u64), String>,
}
impl Reservations {
    fn reserve(&mut self, generation: u64, job: &str, videos: &[Video]) -> Result<(), String> {
        let mut keys = HashSet::new();
        for video in videos {
            let identity: Identity = serde_json::from_str(&video.identity).map_err(err)?;
            let key = (generation, identity.device, identity.inode);
            if !keys.insert(key) {
                return Err("duplicate_selection".into());
            }
            if self.owners.contains_key(&key) {
                return Err("file_busy".into());
            }
        }
        for key in keys {
            self.owners.insert(key, job.into());
        }
        Ok(())
    }
    fn release(&mut self, job: &str) {
        self.owners.retain(|_, owner| owner != job);
    }
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
    reserved: Arc<Mutex<Reservations>>,
    worker: Mutex<Option<thread::JoinHandle<()>>>,
    index: Arc<crate::file_index::IndexService>,
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
        validate_video(&video)?;
        if operation_id.is_empty()
            || operation_id.len() > 128
            || lease.cancelled.load(Ordering::Acquire)
        {
            return Err("discard_cancelled".into());
        }
        let id = uuid::Uuid::new_v4().to_string();
        self.reserved.lock().unwrap().reserve(
            lease.generation,
            &id,
            std::slice::from_ref(&video),
        )?;
        let cancel = Arc::new(AtomicBool::new(false));
        let event = JobEvent {
            discard_operation_id: Some(operation_id.clone()),
            id: id.clone(),
            generation: lease.generation,
            kind: JobKind::Delete,
            status: "queued".into(),
            phase: "preparing".into(),
            name: video.name.clone(),
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
            self.reserved.lock().unwrap().release(&id);
            return Err("file_queue_full".into());
        }
        emit(&self.sink, &event);
        Ok(id)
    }
    pub fn start(resources: PathBuf, sink: EventSink) -> Self {
        let (sender, receiver) = mpsc::sync_channel::<Message>(16);
        let jobs = Arc::new(Mutex::new(HashMap::new()));
        let active = jobs.clone();
        let reserved = Arc::new(Mutex::new(Reservations::default()));
        let worker_reserved = reserved.clone();
        let index = Arc::new(crate::file_index::IndexService::start(sink.clone()));
        let indexing = index.clone();
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
                        worker_reserved.lock().unwrap().release(&id);
                    }
                    Message::List(lease, reply) => {
                        let result = remote
                            .prepare(&lease)
                            .and_then(|_| remote.output(&lease, &["list"], 60_000))
                            .and_then(|bytes| parse_listing(&bytes));
                        let _ = reply.send(result);
                    }
                    Message::Job(mut job) => {
                        let key = TraceKey::new(&job.event.id, job.lease.generation, None);
                        job.trace.mark(&key, Stage::Worker, Edge::Mark);
                        job.event.status = "running".into();
                        emit(&sink, &job.event);
                        let result = check(&job)
                            .and_then(|_| {
                                job.trace
                                    .measure(&key, Stage::Prepare, || remote.prepare(&job.lease))
                            })
                            .and_then(|_| {
                                execute(&remote, &mut job, &sink, &mut sequence, &indexing)
                            });
                        if let Err(error) = result {
                            for video in job.videos.iter().skip(job.event.results.len()) {
                                job.event.results.push(FileResult {
                                    name: video.name.clone(),
                                    status: if error == "operation_cancelled"
                                        || error == "session_replaced"
                                    {
                                        "cancelled"
                                    } else {
                                        "failed"
                                    }
                                    .into(),
                                    message: error.clone(),
                                    requested_name: None,
                                    destination_name: None,
                                });
                            }
                        }
                        job.event.status =
                            settled_status(&job.event.results, job.cancel.load(Ordering::Acquire))
                                .into();
                        job.event.message = if job
                            .event
                            .results
                            .iter()
                            .any(|r| r.status == "outcome_unknown")
                        {
                            "handoff_outcome_unknown".into()
                        } else {
                            job.event
                                .results
                                .iter()
                                .find(|r| r.status != "completed")
                                .map(|r| r.message.clone())
                                .unwrap_or_default()
                        };
                        job.trace.mark(&key, Stage::Terminal, Edge::Mark);
                        active.lock().unwrap().remove(&job.event.id);
                        worker_reserved.lock().unwrap().release(&job.event.id);
                        emit(&sink, &job.event);
                        job.trace.complete_job(&key);
                    }
                    Message::Stop => break,
                }
            }
        });
        Self {
            sender,
            jobs,
            reserved,
            worker: Mutex::new(Some(worker)),
            index,
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
        new_stem: Option<String>,
        on_handoff: Option<HandoffCommit>,
    ) -> Result<String, String> {
        let accepted_at = self.trace.stamp();
        if videos.is_empty() || videos.len() > 10_000 {
            return Err("invalid_selection".into());
        }
        if kind == JobKind::Rename && (videos.len() != 1 || new_stem.is_none()) {
            return Err("rename_requires_one_video".into());
        }
        let mut names = HashSet::new();
        for video in &videos {
            validate_video(video)?;
            if !names.insert(&video.name) {
                return Err("duplicate_selection".into());
            }
        }
        if kind == JobKind::Rename {
            renamed(&videos[0].name, new_stem.as_deref().unwrap())?;
        }
        if lease.cancelled.load(Ordering::Acquire) {
            return Err("session_replaced".into());
        }
        let id = uuid::Uuid::new_v4().to_string();
        self.reserved
            .lock()
            .unwrap()
            .reserve(lease.generation, &id, &videos)?;
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
                new_stem,
                cancel,
                trace: self.trace.clone(),
                on_handoff,
            })))
            .is_err()
        {
            self.jobs.lock().unwrap().remove(&id);
            self.reserved.lock().unwrap().release(&id);
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
            flag.store(true, Ordering::Release);
        }
        if let Some(handle) = self.worker.lock().unwrap().take() {
            let _ = self.sender.send(Message::Stop);
            let _ = handle.join();
        }
        self.index.shutdown();
    }
}
impl Drop for FileService {
    fn drop(&mut self) {
        self.shutdown();
    }
}
fn validate_video(video: &Video) -> Result<(), String> {
    takedock_files::validate_name(&video.name)?;
    let identity: Identity = serde_json::from_str(&video.identity).map_err(err)?;
    if identity.size != video.size || !video.ready {
        return Err("video_not_finalized".into());
    }
    Ok(())
}
fn emit(sink: &EventSink, event: &JobEvent) {
    sink("file-job", serde_json::to_value(event).unwrap());
}
type Remote = crate::helper::HelperClient;
impl Remote {
    fn job_output(&self, job: &Job, args: &[&str], timeout: u64) -> Result<Vec<u8>, String> {
        check(job)?;
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
fn settled_status(results: &[FileResult], cancelled: bool) -> &'static str {
    if !results.is_empty() && results.iter().all(|r| r.status == "completed") {
        "completed"
    } else if results
        .iter()
        .any(|r| matches!(r.status.as_str(), "failed" | "retained" | "outcome_unknown"))
    {
        "failed"
    } else if cancelled || results.iter().any(|r| r.status == "cancelled") {
        "cancelled"
    } else {
        "failed"
    }
}
fn execute(
    remote: &Remote,
    job: &mut Job,
    sink: &EventSink,
    sequence: &mut u64,
    index: &crate::file_index::IndexService,
) -> Result<(), String> {
    use takedock_files::VIDEO_ROOT;
    use takedock_files::handoff::{HANDOFF_ROOT, Outcome};
    for (file_index, video) in job.videos.clone().into_iter().enumerate() {
        check(job)?;
        let key = TraceKey::new(&job.event.id, job.lease.generation, Some(file_index));
        job.event.name = video.name.clone();
        job.event.phase = match job.event.kind {
            JobKind::Mark => "marking",
            JobKind::Delete => "deleting",
            JobKind::Rename => "renaming",
        }
        .into();
        emit(sink, &job.event);
        let requested = if job.event.kind == JobKind::Rename {
            Some(renamed(&video.name, job.new_stem.as_deref().unwrap())?)
        } else {
            None
        };
        let mut result = FileResult {
            name: video.name.clone(),
            status: "failed".into(),
            message: String::new(),
            requested_name: requested.clone(),
            destination_name: None,
        };
        let initial = job
            .trace
            .measure(&key, Stage::InitialStat, || remote.stat(&job.lease, &video));
        if let Err(error) = initial {
            result.message = error;
        } else {
            match job.event.kind {
                JobKind::Mark => {
                    let operation_id = uuid::Uuid::new_v4().to_string();
                    let outcome = job.trace.measure(&key, Stage::Handoff, || {
                        crate::handoff::perform(
                            remote,
                            &job.lease,
                            &video,
                            &operation_id,
                            &job.cancel,
                        )
                    });
                    match outcome {
                        Outcome::HandedOff { receipt, warning } => {
                            result.status = "completed".into();
                            result.destination_name = Some(receipt.destination_name);
                            *sequence = sequence.saturating_add(1);
                            let update = HandoffUpdate {
                                generation: job.lease.generation,
                                job_id: job.event.id.clone(),
                                operation_id,
                                sequence: *sequence,
                                source_name: video.name.clone(),
                                source_identity: video.identity.clone(),
                            };
                            if let Some(callback) = &job.on_handoff {
                                callback(&update);
                            }
                            sink("file-handed-off", serde_json::to_value(update).unwrap());
                            if let Some(code) = warning {
                                sink(
                                    "file-metadata-warning",
                                    serde_json::json!({"generation":job.lease.generation,"id":job.event.id,"code":code}),
                                );
                            }
                            index.enqueue(
                                job.lease.clone(),
                                job.event.id.clone(),
                                vec![
                                    (VIDEO_ROOT, video.name.clone()),
                                    (HANDOFF_ROOT, video.name.clone()),
                                ],
                            );
                        }
                        Outcome::Retained { code } => {
                            result.status = if code == "operation_cancelled" {
                                "cancelled"
                            } else {
                                "retained"
                            }
                            .into();
                            result.message = code;
                        }
                        Outcome::OutcomeUnknown { code } => {
                            result.status = "outcome_unknown".into();
                            result.message = code;
                        }
                    }
                }
                JobKind::Delete => {
                    match job.trace.measure(&key, Stage::Delete, || {
                        remote.job_output(job, &["delete", &video.name, &video.identity], 30_000)
                    }) {
                        Ok(_) => {
                            result.status = "completed".into();
                            index.enqueue(
                                job.lease.clone(),
                                job.event.id.clone(),
                                vec![(VIDEO_ROOT, video.name.clone())],
                            );
                        }
                        Err(error) => result.message = error,
                    }
                }
                JobKind::Rename => {
                    let new_name = requested.as_deref().unwrap();
                    match remote.job_output(
                        job,
                        &["rename", &video.name, new_name, &video.identity],
                        30_000,
                    ) {
                        Err(error) => result.message = error,
                        Ok(_) => {
                            result.status = "completed".into();
                            result.destination_name = Some(new_name.into());
                            let confirmed = remote
                                .output(&job.lease, &["inspect", new_name], 5000)
                                .and_then(|bytes| {
                                    serde_json::from_slice::<Entry>(&bytes).map_err(err)
                                })
                                .and_then(|entry| confirmed_rename_video(&video, new_name, entry));
                            match confirmed {
                                Ok(video_update) => {
                                    *sequence = sequence.saturating_add(1);
                                    sink(
                                        "file-renamed",
                                        serde_json::to_value(RenameUpdate {
                                            generation: job.lease.generation,
                                            id: job.event.id.clone(),
                                            sequence: *sequence,
                                            source_name: video.name.clone(),
                                            source_identity: video.identity.clone(),
                                            video: video_update,
                                        })
                                        .unwrap(),
                                    );
                                }
                                Err(_) => sink(
                                    "file-metadata-warning",
                                    serde_json::json!({"generation":job.lease.generation,"id":job.event.id,"code":"rename_refresh_pending"}),
                                ),
                            }
                            index.enqueue(
                                job.lease.clone(),
                                job.event.id.clone(),
                                vec![
                                    (VIDEO_ROOT, video.name.clone()),
                                    (VIDEO_ROOT, new_name.into()),
                                ],
                            );
                        }
                    }
                }
            }
        }
        if result.status == "completed" {
            job.event.completed += 1;
        }
        job.event.results.push(result);
        emit(sink, &job.event);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn desktop_transfers_are_rejected_and_mark_is_accepted() {
        assert!(serde_json::from_str::<JobKind>("\"copy\"").is_err());
        assert!(serde_json::from_str::<JobKind>("\"move\"").is_err());
        let mark = serde_json::from_str::<JobKind>("\"mark\"").unwrap();
        assert_eq!(
            serde_json::to_value(mark).unwrap(),
            serde_json::json!("mark")
        );
    }
    #[test]
    fn acknowledged_commit_beats_late_cancel_and_uncertainty_beats_cancel() {
        let result = |status: &str| FileResult {
            name: "take.mp4".into(),
            status: status.into(),
            message: String::new(),
            requested_name: None,
            destination_name: None,
        };
        assert_eq!(settled_status(&[result("completed")], true), "completed");
        assert_eq!(
            settled_status(&[result("completed"), result("outcome_unknown")], true),
            "failed"
        );
        assert_eq!(
            settled_status(&[result("completed"), result("cancelled")], true),
            "cancelled"
        );
        assert_eq!(
            settled_status(&[result("retained"), result("completed")], false),
            "failed"
        );
    }
    #[test]
    fn same_identity_cannot_be_reserved_twice() {
        let (video, _) = rename_fixture();
        let mut active = Reservations::default();
        active
            .reserve(7, "first", std::slice::from_ref(&video))
            .unwrap();
        assert_eq!(
            active
                .reserve(7, "second", std::slice::from_ref(&video))
                .unwrap_err(),
            "file_busy"
        );
        active
            .reserve(8, "new-device", std::slice::from_ref(&video))
            .unwrap();
        active.release("first");
        active.reserve(7, "second", &[video]).unwrap();
    }
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
