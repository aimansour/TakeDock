use crate::{
    files::Video,
    model::{RecordingAction, VerificationResult},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU8, Ordering},
    },
};
use takedock_files::{
    Entry, Identity,
    watch::{ManifestEntry, WatchBody, WatchFrame},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TakeToken {
    pub generation: u64,
    pub start_sequence: u64,
    pub id: String,
}
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscardPhase {
    #[default]
    Unavailable,
    Tracking,
    WaitingToStop,
    ConfirmingStop,
    Finalizing,
    Deleting,
    Completed,
    Retained,
    OutcomeUnknown,
}
impl DiscardPhase {
    fn terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Retained | Self::OutcomeUnknown
        )
    }
}
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscardSnapshot {
    pub take: Option<TakeToken>,
    pub eligible: bool,
    pub reason: String,
    pub operation_id: Option<String>,
    pub phase: DiscardPhase,
}
#[derive(Debug, Clone)]
pub struct FileEvidence {
    pub stream: String,
    pub sequence: u64,
    pub complete: bool,
    pub received_ms: u64,
    pub entries: Arc<BTreeMap<String, Identity>>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscardReceipt {
    pub generation: u64,
    pub take: TakeToken,
    pub operation_id: String,
}
#[derive(Debug)]
pub enum DiscardEffect {
    SendStop { operation_id: String },
    Inspect { operation_id: String, name: String },
    Delete { operation_id: String, video: Video },
}
#[derive(Debug, Clone)]
pub enum DeleteOutcome {
    Deleted,
    Retained(String),
    Unknown(String),
}
#[derive(Debug, Clone, Serialize)]
pub struct DiscardResult {
    pub generation: u64,
    pub operation_id: String,
    pub status: String,
    pub code: String,
}
#[derive(Clone)]
struct Take {
    token: TakeToken,
    baseline: Option<FileEvidence>,
    valid: bool,
    reason: String,
    candidate: Option<ManifestEntry>,
    confirmed_ms: Option<u64>,
}
#[derive(Clone)]
struct Operation {
    receipt: DiscardReceipt,
    phase: DiscardPhase,
    code: String,
    deadline: u64,
    verification_ms: u64,
    stop_sequence: Option<u64>,
    inspection_pending: bool,
    next_inspect: u64,
    stable: Option<(Identity, u64)>,
    video: Option<Video>,
    // Shared across recording prediction rollback clones: cancellation is final.
    authorization: Arc<AtomicU8>, // 0 pending, 1 authorized, 2 cancelled
    reported: Arc<AtomicBool>,
}
#[derive(Clone)]
pub struct DiscardMachine {
    generation: u64,
    take: Option<Take>,
    operation: Option<Operation>,
    results: Vec<DiscardResult>,
}
impl DiscardMachine {
    pub(crate) fn observe_state(&mut self, eligible: bool, state: crate::model::RecordingState) {
        let confirmed = self
            .take
            .as_ref()
            .is_some_and(|take| take.confirmed_ms.is_some());
        if !eligible
            || (confirmed && state == crate::model::RecordingState::Unknown)
            || (confirmed
                && state == crate::model::RecordingState::Idle
                && self.operation.is_none())
        {
            self.cancel("discard_observation_lost");
        }
    }
    pub fn new(generation: u64) -> Self {
        Self {
            generation,
            take: None,
            operation: None,
            results: Vec::new(),
        }
    }
    pub fn start_queued(
        &mut self,
        take: TakeToken,
        evidence: Option<FileEvidence>,
        confirmed_idle: bool,
        now_ms: u64,
    ) {
        self.cancel("discard_take_replaced");
        let reason = if take.generation != self.generation || !confirmed_idle {
            "discard_not_confirmed_idle"
        } else if !evidence.as_ref().is_some_and(|baseline| {
            baseline.complete && now_ms.saturating_sub(baseline.received_ms) <= 2500
        }) {
            "discard_baseline_unavailable"
        } else {
            ""
        };
        self.take = Some(Take {
            token: take,
            baseline: evidence,
            valid: reason.is_empty(),
            reason: reason.into(),
            candidate: None,
            confirmed_ms: None,
        });
    }
    pub fn file_frame(&mut self, frame: &WatchFrame, now_ms: u64) {
        let Some(take) = &mut self.take else {
            return;
        };
        if !take.valid {
            return;
        }
        let Some(baseline) = &mut take.baseline else {
            return;
        };
        if frame.version != 1
            || frame.stream != baseline.stream
            || frame.sequence != baseline.sequence.saturating_add(1)
        {
            self.cancel("discard_file_history_lost");
            return;
        }
        baseline.sequence = frame.sequence;
        baseline.received_ms = now_ms;
        let valid = match &frame.body {
            WatchBody::Heartbeat => true,
            WatchBody::Created { entry } => {
                let video = entry.name.rsplit('.').next().is_some_and(|extension| {
                    ["mp4", "3gp", "m4v", "mov", "webm", "mkv"]
                        .contains(&extension.to_lowercase().as_str())
                });
                if !video || baseline.entries.contains_key(&entry.name) || take.candidate.is_some()
                {
                    false
                } else {
                    take.candidate = Some(entry.clone());
                    true
                }
            }
            WatchBody::Changed { entry } | WatchBody::Closed { entry } => {
                if let Some(candidate) = &mut take.candidate {
                    if same_object(candidate, entry) {
                        *candidate = entry.clone();
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            _ => false,
        };
        if !valid {
            self.cancel("discard_file_history_lost");
        }
    }
    pub fn verification(&mut self, result: &VerificationResult, now_ms: u64) {
        if result.generation != self.generation {
            return;
        }
        if let Some(take) = &mut self.take
            && result.sequence == take.token.start_sequence
            && result.action == RecordingAction::Start
        {
            if result.status == "confirmed" && take.valid {
                take.confirmed_ms.get_or_insert(now_ms);
            } else if ["failed", "unconfirmed", "superseded"].contains(&result.status.as_str()) {
                self.cancel("discard_start_unconfirmed");
            }
        }
        let Some(operation) = &mut self.operation else {
            return;
        };
        if operation.phase == DiscardPhase::ConfirmingStop
            && operation.stop_sequence == Some(result.sequence)
            && result.action == RecordingAction::Stop
        {
            if result.status == "confirmed" {
                operation.phase = DiscardPhase::Finalizing;
                operation.deadline = now_ms.saturating_add(10_000);
                operation.next_inspect = now_ms;
            } else if ["failed", "unconfirmed", "superseded"].contains(&result.status.as_str()) {
                self.cancel("discard_stop_unconfirmed");
            }
        }
    }
    pub fn request(
        &mut self,
        take: &TakeToken,
        now_ms: u64,
        verification_ms: u64,
    ) -> Result<DiscardReceipt, String> {
        if take.generation != self.generation
            || self
                .take
                .as_ref()
                .is_none_or(|current| current.token != *take)
        {
            return Err("discard_take_replaced".into());
        }
        if let Some(operation) = &self.operation {
            if operation.receipt.take == *take {
                return Ok(operation.receipt.clone());
            }
            if !operation.phase.terminal() {
                return Err("discard_busy".into());
            }
        }
        if !self.take.as_ref().unwrap().valid {
            return Err(self.take.as_ref().unwrap().reason.clone());
        }
        let receipt = DiscardReceipt {
            generation: self.generation,
            take: take.clone(),
            operation_id: uuid::Uuid::new_v4().to_string(),
        };
        self.operation = Some(Operation {
            receipt: receipt.clone(),
            phase: DiscardPhase::WaitingToStop,
            code: String::new(),
            deadline: now_ms.saturating_add(verification_ms),
            verification_ms,
            stop_sequence: None,
            inspection_pending: false,
            next_inspect: now_ms,
            stable: None,
            video: None,
            authorization: Arc::new(AtomicU8::new(0)),
            reported: Arc::new(AtomicBool::new(false)),
        });
        Ok(receipt)
    }
    pub fn ordinary_intent(&mut self, action: RecordingAction) {
        if matches!(action, RecordingAction::Start | RecordingAction::Stop)
            || self
                .operation
                .as_ref()
                .is_some_and(|operation| !operation.phase.terminal())
        {
            self.cancel("discard_capture_changed");
        }
    }
    pub fn stop_queued(&mut self, operation_id: &str, sequence: u64) {
        if let Some(operation) = &mut self.operation
            && operation.receipt.operation_id == operation_id
            && operation.phase == DiscardPhase::ConfirmingStop
        {
            operation.stop_sequence = Some(sequence);
        }
    }
    pub fn inspected(&mut self, operation_id: &str, entry: Result<Entry, String>, now_ms: u64) {
        let Some(operation) = &mut self.operation else {
            return;
        };
        if operation.receipt.operation_id != operation_id
            || operation.phase != DiscardPhase::Finalizing
            || !operation.inspection_pending
        {
            return;
        }
        operation.inspection_pending = false;
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                self.cancel("discard_inspection_failed");
                return;
            }
        };
        let candidate = self.take.as_ref().and_then(|take| take.candidate.as_ref());
        if candidate.is_none_or(|candidate| {
            candidate.name != entry.name
                || candidate.identity.device != entry.identity.device
                || candidate.identity.inode != entry.identity.inode
        }) {
            self.cancel("discard_source_changed");
            return;
        }
        if !entry.ready || entry.duration_ms.is_none() {
            operation.stable = None;
            operation.next_inspect = now_ms.saturating_add(250);
            return;
        }
        if let Some((previous, first_ms)) = &operation.stable {
            if *previous != entry.identity {
                self.cancel("discard_source_changed");
                return;
            }
            if now_ms.saturating_sub(*first_ms) < 250 {
                operation.next_inspect = first_ms.saturating_add(250);
                return;
            }
            let Ok(modified_ms) = u64::try_from(entry.identity.modified_ns / 1_000_000) else {
                self.cancel("discard_source_changed");
                return;
            };
            operation.video = Some(Video {
                name: entry.name,
                size: entry.identity.size,
                modified_ms,
                ready: true,
                duration_ms: entry.duration_ms,
                identity: serde_json::to_string(&entry.identity).unwrap(),
            });
        } else {
            operation.stable = Some((entry.identity, now_ms));
            operation.next_inspect = now_ms.saturating_add(250);
        }
    }
    pub fn poll(&mut self, now_ms: u64) -> Vec<DiscardEffect> {
        if self.take.as_ref().is_some_and(|take| {
            take.valid
                && take
                    .baseline
                    .as_ref()
                    .is_none_or(|baseline| now_ms.saturating_sub(baseline.received_ms) > 2500)
        }) {
            self.cancel("discard_file_history_lost");
        }
        let Some(operation) = &mut self.operation else {
            return Vec::new();
        };
        if operation.phase.terminal() {
            return Vec::new();
        }
        if operation.authorization.load(Ordering::Acquire) == 2 {
            self.cancel("discard_cancelled");
            return Vec::new();
        }
        if matches!(
            operation.phase,
            DiscardPhase::WaitingToStop | DiscardPhase::ConfirmingStop | DiscardPhase::Finalizing
        ) && now_ms >= operation.deadline
        {
            let code = if operation.phase == DiscardPhase::Finalizing {
                "discard_finalization_timeout"
            } else {
                "discard_stop_unconfirmed"
            };
            self.cancel(code);
            return Vec::new();
        }
        let id = operation.receipt.operation_id.clone();
        match operation.phase {
            DiscardPhase::WaitingToStop => {
                if self
                    .take
                    .as_ref()
                    .and_then(|take| take.confirmed_ms)
                    .is_some_and(|confirmed| now_ms >= confirmed.saturating_add(600))
                {
                    operation.phase = DiscardPhase::ConfirmingStop;
                    operation.deadline = now_ms.saturating_add(operation.verification_ms);
                    vec![DiscardEffect::SendStop { operation_id: id }]
                } else {
                    Vec::new()
                }
            }
            DiscardPhase::Finalizing => {
                if let Some(video) = operation.video.take() {
                    operation.phase = DiscardPhase::Deleting;
                    return vec![DiscardEffect::Delete {
                        operation_id: id,
                        video,
                    }];
                }
                if !operation.inspection_pending
                    && now_ms >= operation.next_inspect
                    && let Some(candidate) =
                        self.take.as_ref().and_then(|take| take.candidate.as_ref())
                {
                    operation.inspection_pending = true;
                    vec![DiscardEffect::Inspect {
                        operation_id: id,
                        name: candidate.name.clone(),
                    }]
                } else {
                    Vec::new()
                }
            }
            _ => Vec::new(),
        }
    }
    pub fn cancel(&mut self, code: &str) {
        if let Some(take) = &mut self.take {
            take.valid = false;
            take.reason = code.into();
        }
        if let Some(operation) = &mut self.operation {
            if operation.phase.terminal() || operation.authorization.load(Ordering::Acquire) == 1 {
                return;
            }
            operation.authorization.store(2, Ordering::Release);
            operation.phase = DiscardPhase::Retained;
            operation.code = code.into();
            self.report();
        }
    }
    pub fn authorize_delete(&mut self, operation_id: &str) -> bool {
        let Some(operation) = &self.operation else {
            return false;
        };
        operation.receipt.operation_id == operation_id
            && operation.phase == DiscardPhase::Deleting
            && self
                .take
                .as_ref()
                .is_some_and(|take| take.valid && take.token == operation.receipt.take)
            && operation
                .authorization
                .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
    }
    pub(crate) fn delete_pending(&self, operation_id: &str) -> bool {
        self.operation.as_ref().is_some_and(|operation| {
            operation.receipt.operation_id == operation_id
                && operation.phase == DiscardPhase::Deleting
                && operation.authorization.load(Ordering::Acquire) == 0
                && self
                    .take
                    .as_ref()
                    .is_some_and(|take| take.valid && take.token == operation.receipt.take)
        })
    }
    pub fn finish_delete(&mut self, operation_id: &str, outcome: DeleteOutcome) {
        let Some(operation) = &mut self.operation else {
            return;
        };
        if operation.receipt.operation_id != operation_id || operation.phase.terminal() {
            return;
        }
        if !matches!(outcome, DeleteOutcome::Retained(_))
            && operation.authorization.load(Ordering::Acquire) != 1
        {
            return;
        }
        match outcome {
            DeleteOutcome::Deleted => {
                operation.phase = DiscardPhase::Completed;
                operation.code = "discard_completed".into();
            }
            DeleteOutcome::Retained(code) => {
                operation.phase = DiscardPhase::Retained;
                operation.code = code;
            }
            DeleteOutcome::Unknown(code) => {
                operation.phase = DiscardPhase::OutcomeUnknown;
                operation.code = code;
            }
        }
        self.report();
    }
    fn report(&mut self) {
        let Some(operation) = &self.operation else {
            return;
        };
        if operation.reported.swap(true, Ordering::AcqRel) {
            return;
        }
        self.results.push(DiscardResult {
            generation: operation.receipt.generation,
            operation_id: operation.receipt.operation_id.clone(),
            status: match operation.phase {
                DiscardPhase::Completed => "completed",
                DiscardPhase::OutcomeUnknown => "outcome_unknown",
                _ => "retained",
            }
            .into(),
            code: operation.code.clone(),
        });
    }
    pub fn drain_results(&mut self) -> Vec<DiscardResult> {
        std::mem::take(&mut self.results)
    }
    pub fn snapshot(&self) -> DiscardSnapshot {
        let take = self.take.as_ref();
        let operation = self.operation.as_ref();
        let operation_current = operation
            .is_some_and(|operation| take.is_some_and(|take| take.token == operation.receipt.take));
        let busy = operation.is_some_and(|operation| !operation.phase.terminal());
        let phase = if operation_current || busy {
            let operation = operation.unwrap();
            if !operation.phase.terminal() && operation.authorization.load(Ordering::Acquire) == 2 {
                DiscardPhase::Retained
            } else {
                operation.phase
            }
        } else if take.is_some_and(|take| take.valid) {
            DiscardPhase::Tracking
        } else {
            DiscardPhase::Unavailable
        };
        DiscardSnapshot {
            take: take.map(|take| take.token.clone()),
            eligible: take.is_some_and(|take| take.valid) && !operation_current && !busy,
            reason: if operation_current {
                operation.unwrap().code.clone()
            } else {
                take.map(|take| take.reason.clone())
                    .unwrap_or_else(|| "discard_no_managed_take".into())
            },
            operation_id: operation
                .filter(|_| operation_current || busy)
                .map(|operation| operation.receipt.operation_id.clone()),
            phase,
        }
    }
}
fn same_object(a: &ManifestEntry, b: &ManifestEntry) -> bool {
    a.name == b.name
        && a.identity.device == b.identity.device
        && a.identity.inode == b.identity.inode
}

#[cfg(test)]
mod tests;
