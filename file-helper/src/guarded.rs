use crate::{Identity, Root};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, Read, Write};

pub const MAX_MESSAGE: usize = 4096;
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
        status: String,
        code: String,
    },
}
pub fn write_message(output: &mut impl Write, message: &Message) -> Result<(), String> {
    serde_json::to_writer(&mut *output, message).map_err(|e| e.to_string())?;
    output
        .write_all(b"\n")
        .and_then(|_| output.flush())
        .map_err(|e| e.to_string())
}
pub fn read_permit_stdin() -> Result<Vec<u8>, String> {
    let (send, receive) = std::sync::mpsc::channel();
    // This helper command is a single-use process. A timed-out stdin reader
    // cannot outlive the process or perform any mutation.
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = std::io::stdin()
            .lock()
            .take((MAX_MESSAGE + 1) as u64)
            .read_until(b'\n', &mut bytes)
            .map_err(|_| "discard_permit_failed".to_string())
            .and_then(|_| {
                if bytes.len() > MAX_MESSAGE || bytes.last() != Some(&b'\n') {
                    Err("discard_permit_failed".into())
                } else {
                    Ok(bytes)
                }
            });
        let _ = send.send(result);
    });
    receive
        .recv_timeout(std::time::Duration::from_millis(5000))
        .map_err(|_| "discard_permit_timeout".to_string())?
}
pub fn serve(
    root: &Root,
    name: &str,
    expected: &Identity,
    operation_id: &str,
    read_permit: impl FnOnce() -> Result<Vec<u8>, String>,
    output: &mut impl Write,
) -> Result<(), String> {
    if operation_id.is_empty() || operation_id.len() > 128 {
        return Err("discard_invalid_operation".into());
    }
    let mut permitted = false;
    let result = root.delete_guarded(name, expected, || {
        write_message(
            output,
            &Message::Prepared {
                operation_id: operation_id.into(),
            },
        )?;
        let bytes = read_permit()?;
        if bytes.len() > MAX_MESSAGE {
            return Err("discard_permit_failed".into());
        }
        let permit: Message =
            serde_json::from_slice(&bytes).map_err(|_| "discard_permit_failed")?;
        match permit {
            Message::Permit {
                operation_id: supplied,
                allow,
            } if supplied == operation_id => {
                permitted = allow;
                Ok(allow)
            }
            _ => Err("discard_permit_failed".into()),
        }
    });
    match result {
        Ok(()) => write_message(
            output,
            &Message::Result {
                operation_id: operation_id.into(),
                status: "deleted".into(),
                code: "discard_completed".into(),
            },
        ),
        Err(error) if !permitted => write_message(
            output,
            &Message::Result {
                operation_id: operation_id.into(),
                status: "retained".into(),
                code: error,
            },
        ),
        // A post-permit error might follow unlink (e.g. journal cleanup).
        // Closing without a result forces the host to report uncertainty.
        Err(error) => Err(error),
    }
}
