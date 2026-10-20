use crate::{
    discard::DeleteOutcome,
    files::{FileResult, JobEvent, Video},
    helper::HelperClient,
    session::{DeviceLease, EventSink},
};
use std::{
    io::{BufRead, BufReader, Read},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
use takedock_files::guarded::{self, Message};

pub(crate) struct DiscardJob {
    pub event: JobEvent,
    pub lease: DeviceLease,
    pub video: Video,
    pub operation_id: String,
    pub cancel: Arc<AtomicBool>,
    pub should_prepare: Arc<dyn Fn() -> bool + Send + Sync>,
    pub authorize: Arc<dyn Fn() -> bool + Send + Sync>,
    pub complete: Arc<dyn Fn(DeleteOutcome) + Send + Sync>,
}
fn emit(sink: &EventSink, event: &JobEvent) {
    sink("file-job", serde_json::to_value(event).unwrap());
}
pub(crate) fn run(helper: &mut HelperClient, mut job: DiscardJob, sink: &EventSink) {
    job.event.status = "running".into();
    emit(sink, &job.event);
    let outcome = if job.cancel.load(Ordering::Acquire)
        || job.lease.cancelled.load(Ordering::Acquire)
        || !(job.should_prepare)()
    {
        DeleteOutcome::Retained("discard_cancelled".into())
    } else {
        match helper.prepare(&job.lease) {
            Ok(()) => perform(helper, &mut job, sink),
            Err(_) => DeleteOutcome::Retained("discard_delete_failed".into()),
        }
    };
    let (status, code) = match &outcome {
        DeleteOutcome::Deleted => ("completed", "discard_completed"),
        DeleteOutcome::Retained(code) => ("cancelled", code.as_str()),
        DeleteOutcome::Unknown(_) => ("failed", "discard_outcome_unknown"),
    };
    job.event.status = status.into();
    job.event.phase = status.into();
    job.event.message = code.into();
    job.event.completed = usize::from(matches!(outcome, DeleteOutcome::Deleted));
    job.event.results.push(FileResult {
        name: job.video.name.clone(),
        status: status.into(),
        message: code.into(),
    });
    if matches!(outcome, DeleteOutcome::Deleted) {
        let encoded = job
            .video
            .name
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
            sink(
                "file-index-warning",
                serde_json::json!({"generation":job.lease.generation,"id":job.event.id,"code":"media_index_pending"}),
            );
        }
    }
    (job.complete)(outcome);
    emit(sink, &job.event);
}
fn uncertain(authorized: bool, code: &str) -> DeleteOutcome {
    if authorized {
        DeleteOutcome::Unknown("discard_outcome_unknown".into())
    } else {
        DeleteOutcome::Retained(code.into())
    }
}
fn perform(helper: &HelperClient, job: &mut DiscardJob, sink: &EventSink) -> DeleteOutcome {
    let command = match helper.command(&[
        "delete-guarded",
        &job.video.name,
        &job.video.identity,
        &job.operation_id,
    ]) {
        Ok(command) => command,
        Err(_) => return DeleteOutcome::Retained("discard_delete_failed".into()),
    };
    let mut child =
        match job
            .lease
            .adb
            .spawn(Some(&job.lease.serial), &["shell", "-T", &command], true)
        {
            Ok(child) => child,
            Err(_) => return DeleteOutcome::Retained("discard_delete_failed".into()),
        };
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let errors = child.stderr.take().unwrap();
    let (sender, receiver) = mpsc::sync_channel(4);
    let reader = thread::spawn(move || {
        loop {
            let mut line = Vec::new();
            let result = output
                .by_ref()
                .take((guarded::MAX_MESSAGE + 1) as u64)
                .read_until(b'\n', &mut line);
            if result.is_err() || line.is_empty() {
                break;
            }
            if line.len() > guarded::MAX_MESSAGE || line.last() != Some(&b'\n') {
                let _ = sender.send(Err(()));
                break;
            }
            let message = serde_json::from_slice::<Message>(&line).map_err(|_| ());
            if sender.send(message).is_err() {
                break;
            }
        }
    });
    let stderr = thread::spawn(move || {
        let _ = std::io::copy(&mut errors.take(64 * 1024), &mut std::io::sink());
    });
    let started = Instant::now();
    let mut authorized = false;
    let mut prepared = false;
    let outcome = loop {
        if started.elapsed() >= Duration::from_secs(30)
            || job.lease.cancelled.load(Ordering::Acquire)
        {
            break uncertain(authorized, "discard_delete_failed");
        }
        if !authorized && job.cancel.load(Ordering::Acquire) {
            break DeleteOutcome::Retained("discard_cancelled".into());
        }
        let message = match receiver.recv_timeout(Duration::from_millis(25)) {
            Ok(Ok(message)) => message,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            _ => break uncertain(authorized, "discard_delete_failed"),
        };
        match message {
            Message::Prepared { operation_id } if operation_id == job.operation_id && !prepared => {
                prepared = true;
                authorized = !job.cancel.load(Ordering::Acquire)
                    && !job.lease.cancelled.load(Ordering::Acquire)
                    && (job.authorize)();
                let permit = Message::Permit {
                    operation_id,
                    allow: authorized,
                };
                if guarded::write_message(&mut input, &permit).is_err() {
                    break uncertain(authorized, "discard_delete_failed");
                }
                if authorized {
                    job.event.phase = "deleting".into();
                    emit(sink, &job.event);
                }
            }
            Message::Result {
                operation_id,
                status,
                code,
            } if operation_id == job.operation_id => {
                break match status.as_str() {
                    "deleted" if authorized => DeleteOutcome::Deleted,
                    "retained" => DeleteOutcome::Retained(code),
                    _ => uncertain(authorized, "discard_delete_failed"),
                };
            }
            _ => break uncertain(authorized, "discard_delete_failed"),
        }
    };
    drop(input);
    let _ = child.kill();
    let _ = child.wait();
    drop(receiver);
    let _ = reader.join();
    let _ = stderr.join();
    outcome
}
