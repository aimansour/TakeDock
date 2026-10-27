use crate::{files::Video, helper::HelperClient, session::DeviceLease};
use std::{
    io::{BufRead, BufReader, Read},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
use takedock_files::{
    Identity,
    handoff::{self, Message, Outcome},
};

fn lost_result(permitted: bool) -> Outcome {
    if permitted {
        Outcome::OutcomeUnknown {
            code: "handoff_outcome_unknown".into(),
        }
    } else {
        Outcome::Retained {
            code: "handoff_failed".into(),
        }
    }
}
fn confirm_outcome(permitted: bool, video: &Video, operation: &str, outcome: Outcome) -> Outcome {
    match outcome {
        Outcome::HandedOff { receipt, warning } => {
            let expected = serde_json::from_str::<Identity>(&video.identity).ok();
            if permitted
                && receipt.operation_id == operation
                && receipt.source_name == video.name
                && receipt.destination_name == video.name
                && expected.is_some_and(|expected| {
                    receipt.identity.device == expected.device
                        && receipt.identity.inode == expected.inode
                        && receipt.identity.size == expected.size
                        && receipt.identity.modified_ns == expected.modified_ns
                })
            {
                Outcome::HandedOff { receipt, warning }
            } else {
                lost_result(permitted)
            }
        }
        other => other,
    }
}
pub(crate) fn perform(
    helper: &HelperClient,
    lease: &DeviceLease,
    video: &Video,
    operation: &str,
    cancel: &AtomicBool,
) -> Outcome {
    if cancel.load(Ordering::Acquire) || lease.cancelled.load(Ordering::Acquire) {
        return Outcome::Retained {
            code: "operation_cancelled".into(),
        };
    }
    let command =
        match helper.command(&["handoff-guarded", &video.name, &video.identity, operation]) {
            Ok(command) => command,
            Err(_) => return lost_result(false),
        };
    let mut child = match lease
        .adb
        .spawn(Some(&lease.serial), &["shell", "-T", &command], true)
    {
        Ok(child) => child,
        Err(_) => return lost_result(false),
    };
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let errors = child.stderr.take().unwrap();
    let (sender, receiver) = mpsc::sync_channel(4);
    let reader = thread::spawn(move || {
        loop {
            let mut line = Vec::new();
            let read = output
                .by_ref()
                .take((handoff::MAX_MESSAGE + 1) as u64)
                .read_until(b'\n', &mut line);
            if read.is_err() || line.is_empty() {
                break;
            }
            if line.len() > handoff::MAX_MESSAGE || line.last() != Some(&b'\n') {
                let _ = sender.send(Err(()));
                break;
            }
            if sender
                .send(serde_json::from_slice::<Message>(&line).map_err(|_| ()))
                .is_err()
            {
                break;
            }
        }
    });
    let stderr = thread::spawn(move || {
        let _ = std::io::copy(&mut errors.take(64 * 1024), &mut std::io::sink());
    });
    let started = Instant::now();
    let mut permitted = false;
    let mut prepared = false;
    let outcome = loop {
        if started.elapsed() >= Duration::from_secs(30) || lease.cancelled.load(Ordering::Acquire) {
            break lost_result(permitted);
        }
        if !permitted && cancel.load(Ordering::Acquire) {
            break Outcome::Retained {
                code: "operation_cancelled".into(),
            };
        }
        let message = match receiver.recv_timeout(Duration::from_millis(25)) {
            Ok(Ok(message)) => message,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            _ => break lost_result(permitted),
        };
        match message {
            Message::Prepared { operation_id } if operation_id == operation && !prepared => {
                prepared = true;
                let allow =
                    !cancel.load(Ordering::Acquire) && !lease.cancelled.load(Ordering::Acquire);
                // From the first write attempt, a lost ack cannot prove retention.
                permitted = allow;
                if handoff::write_message(
                    &mut input,
                    &Message::Permit {
                        operation_id,
                        allow,
                    },
                )
                .is_err()
                {
                    break lost_result(permitted);
                }
            }
            Message::Result {
                operation_id,
                outcome,
            } if operation_id == operation => {
                break confirm_outcome(permitted, video, operation, outcome);
            }
            _ => break lost_result(permitted),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::files::Video;
    use takedock_files::{
        Identity,
        handoff::{Outcome, Receipt},
    };
    fn sample() -> (Video, Receipt) {
        let identity = Identity {
            device: 9007199254740993,
            inode: 9007199254740995,
            size: 100,
            modified_ns: 1700000000123456789,
            changed_ns: 1700000000123456790,
        };
        let video = Video {
            name: "رحلة '1.mp4".into(),
            size: 100,
            modified_ms: 1700000000123,
            ready: true,
            duration_ms: Some(100),
            identity: serde_json::to_string(&identity).unwrap(),
        };
        let mut moved = identity;
        moved.changed_ns += 1;
        let receipt = Receipt {
            operation_id: "operation-1".into(),
            source_name: video.name.clone(),
            destination_name: video.name.clone(),
            identity: moved,
        };
        (video, receipt)
    }
    #[test]
    fn confirmed_receipt_only_removes_exact_source() {
        let (video, receipt) = sample();
        for field in [
            "operation",
            "source",
            "destination",
            "device",
            "inode",
            "size",
            "modified",
        ] {
            let mut bad = receipt.clone();
            match field {
                "operation" => bad.operation_id.push('x'),
                "source" => bad.source_name.push('x'),
                "destination" => bad.destination_name.push('x'),
                "device" => bad.identity.device += 1,
                "inode" => bad.identity.inode += 1,
                "size" => bad.identity.size += 1,
                _ => bad.identity.modified_ns += 1,
            }
            assert!(
                matches!(
                    confirm_outcome(
                        true,
                        &video,
                        "operation-1",
                        Outcome::HandedOff {
                            receipt: bad,
                            warning: None
                        }
                    ),
                    Outcome::OutcomeUnknown { .. }
                ),
                "{field}"
            );
        }
        assert!(matches!(
            confirm_outcome(
                true,
                &video,
                "operation-1",
                Outcome::HandedOff {
                    receipt,
                    warning: None
                }
            ),
            Outcome::HandedOff { .. }
        ));
    }
    #[test]
    fn ack_lost_after_permit_is_unknown() {
        assert!(matches!(lost_result(false), Outcome::Retained { .. }));
        assert!(matches!(lost_result(true), Outcome::OutcomeUnknown { .. }));
        let (video, receipt) = sample();
        assert!(!matches!(
            confirm_outcome(
                false,
                &video,
                "operation-1",
                Outcome::HandedOff {
                    receipt,
                    warning: None
                }
            ),
            Outcome::HandedOff { .. }
        ));
    }
}
