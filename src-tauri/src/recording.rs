use crate::discard::{DiscardMachine, DiscardPhase, FileEvidence, TakeToken};
use crate::model::*;
use crate::readiness::{Readiness, ReadinessPolicy, ReadyToken};
use std::collections::VecDeque;
use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};

pub fn transition(
    state: RecordingState,
    action: RecordingAction,
) -> Result<RecordingState, String> {
    use RecordingAction::*;
    use RecordingState::*;
    match (state, action) {
        (Idle, Start) | (Paused, Resume) => Ok(Recording),
        (Recording, Pause) => Ok(Paused),
        (Recording | Paused, Stop) => Ok(Idle),
        _ => Err("invalid_recording_transition".into()),
    }
}
#[derive(Clone)]
struct Pending {
    discard_operation_id: Option<String>,
    write_ticket: Option<Arc<AtomicU8>>,
    sequence: u64,
    action: RecordingAction,
    suppress_success_sound: bool,
    expected: RecordingState,
    boundary: u64,
    deadline: u64,
}
#[derive(Clone)]
pub struct RecordingMachine {
    pub(crate) discard: DiscardMachine,
    pub(crate) file_evidence: Option<FileEvidence>,
    pub state: SessionState,
    session: String,
    observation_sequence: u64,
    event_time: u64,
    pending: VecDeque<Pending>,
    capture_available: bool,
    readiness: Readiness,
    pub(crate) readiness_policy: ReadinessPolicy,
}
impl RecordingMachine {
    pub(crate) fn prepare_intent(&mut self, action: RecordingAction) {
        let operations: Vec<_> = self
            .pending
            .iter()
            .filter_map(|pending| pending.discard_operation_id.clone())
            .collect();
        for operation in operations {
            self.withdraw_discard_stop(&operation);
        }
        self.discard.ordinary_intent(action);
        self.invalidate_speech_readiness();
    }
    pub(crate) fn withdraw_discard_stop(&mut self, operation_id: &str) -> Option<u64> {
        let index = self.pending.iter().position(|pending| {
            pending.discard_operation_id.as_deref() == Some(operation_id)
                && pending.write_ticket.as_ref().is_some_and(|ticket| {
                    ticket
                        .compare_exchange(0, 2, Ordering::AcqRel, Ordering::Acquire)
                        .is_ok()
                })
        })?;
        let sequence = self.pending.remove(index)?.sequence;
        self.state.pending = self.pending.len();
        self.state.predicted = self
            .pending
            .back()
            .map(|pending| pending.expected)
            .unwrap_or(self.state.observed);
        Some(sequence)
    }
    pub(crate) fn discard_write_ticket(&self, sequence: u64) -> Option<Arc<AtomicU8>> {
        self.pending
            .iter()
            .find(|pending| pending.sequence == sequence)
            .and_then(|pending| pending.write_ticket.clone())
    }
    pub(crate) fn commit_discard_write(&mut self, sequence: u64, ticket: &Arc<AtomicU8>) -> bool {
        let current = self.discard.snapshot();
        self.pending.iter().any(|pending| {
            pending.sequence == sequence && pending.discard_operation_id == current.operation_id
        }) && current.phase == DiscardPhase::ConfirmingStop
            && ticket
                .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
    }
    pub(crate) fn withdraw_cancelled_discard(&mut self) {
        let snapshot = self.discard.snapshot();
        if matches!(
            snapshot.phase,
            DiscardPhase::Retained | DiscardPhase::Unavailable
        ) && let Some(operation) = snapshot.operation_id
        {
            self.withdraw_discard_stop(&operation);
        }
    }
    pub(crate) fn cancel_discard(&mut self, code: &str) {
        self.discard.cancel(code);
        self.withdraw_cancelled_discard();
    }
    pub fn snapshot_for_event(&mut self) -> SessionState {
        self.state.discard = self.discard.snapshot();
        if self.state.discard.take.is_none()
            && self.state.observed == RecordingState::Idle
            && self
                .file_evidence
                .as_ref()
                .is_some_and(|evidence| evidence.complete)
        {
            self.state.discard.reason = "discard_ready_for_start".into();
        }
        self.state.revision += 1;
        self.state.clone()
    }
    pub fn new(session: String, generation: u64) -> Self {
        let defaults = Settings::default();
        Self {
            discard: DiscardMachine::new(generation),
            file_evidence: None,
            state: SessionState {
                generation,
                connected: true,
                condition: "observer_starting".into(),
                ..Default::default()
            },
            session,
            observation_sequence: 0,
            event_time: 0,
            pending: VecDeque::new(),
            capture_available: true,
            readiness: Readiness::new(generation),
            readiness_policy: ReadinessPolicy {
                sound: defaults.ready_sound,
                delay_ms: defaults.ready_delay_ms,
            },
        }
    }
    pub fn observe(&mut self, observation: Observation, now_ms: u64) -> Vec<VerificationResult> {
        if observation.version != 1
            || observation.session != self.session
            || observation.seq <= self.observation_sequence
            || observation.event_time < self.event_time
        {
            return Vec::new();
        }
        self.observation_sequence = observation.seq;
        self.event_time = observation.event_time;
        if !self.capture_available {
            self.state.observed = observation.state;
            return Vec::new();
        }
        self.state.observer_ready = true;
        self.state.foreground = observation.foreground;
        self.state.video_mode = observation.video_mode;
        self.state.observed = observation.state;
        self.state.condition = if !observation.foreground {
            "open_camera_foreground"
        } else if !observation.video_mode {
            "select_video_mode"
        } else if observation.state == RecordingState::Unknown {
            "unsupported_camera_state"
        } else {
            "ready"
        }
        .into();
        let mut results = Vec::new();
        // The earliest matching intent owns the evidence. In start/pause/resume,
        // a delayed recording state proves start, never the later resume.
        // Missing distinct intermediate states may coalesce; repeated matching
        // states remain unconfirmed until ordered evidence or deadline expiry.
        if let Some(index) = self.pending.iter().position(|pending| {
            pending.boundary < observation.seq && pending.expected == observation.state
        }) {
            for position in 0..=index {
                let pending = self.pending.pop_front().unwrap();
                let status = if position == index {
                    "confirmed"
                } else {
                    "superseded"
                };
                results.push(VerificationResult {
                    discard_operation_id: pending.discard_operation_id,
                    generation: self.state.generation,
                    sequence: pending.sequence,
                    action: pending.action,
                    suppress_success_sound: pending.suppress_success_sound,
                    status: status.into(),
                    message: String::new(),
                });
            }
        }
        self.state.pending = self.pending.len();
        if self.pending.is_empty() {
            self.state.predicted = observation.state;
        }
        for result in &results {
            self.discard.verification(result, now_ms);
            if result.action == RecordingAction::Start && result.status == "confirmed" {
                self.readiness.confirm(result.sequence, now_ms);
            }
        }
        self.reconcile_readiness();
        self.discard.observe_state(
            self.state.connected
                && self.state.observer_ready
                && self.state.foreground
                && self.state.video_mode,
            self.state.observed,
        );
        self.withdraw_cancelled_discard();
        results
    }
    pub fn dispatch(
        &mut self,
        action: RecordingAction,
        now: u64,
        timeout: u64,
    ) -> Result<CommandReceipt, String> {
        self.prepare_intent(action);
        self.dispatch_raw(action, now, timeout, None)
    }
    pub(crate) fn dispatch_raw(
        &mut self,
        action: RecordingAction,
        now: u64,
        timeout: u64,
        discard_operation_id: Option<&str>,
    ) -> Result<CommandReceipt, String> {
        if !self.state.connected
            || !self.state.observer_ready
            || !self.state.foreground
            || !self.state.video_mode
        {
            return Err("camera_not_eligible".into());
        }
        self.invalidate_speech_readiness();
        if let Some(operation) = discard_operation_id {
            let snapshot = self.discard.snapshot();
            if action != RecordingAction::Stop
                || snapshot.operation_id.as_deref() != Some(operation)
                || snapshot.phase != DiscardPhase::ConfirmingStop
            {
                return Err("discard_cancelled".into());
            }
        }
        let confirmed_idle = self.state.observed == RecordingState::Idle
            && self.state.predicted == RecordingState::Idle
            && self.pending.is_empty();
        let expected = transition(self.state.predicted, action)?;
        self.state.command_sequence += 1;
        self.pending.push_back(Pending {
            discard_operation_id: discard_operation_id.map(str::to_owned),
            write_ticket: discard_operation_id.map(|_| Arc::new(AtomicU8::new(0))),
            sequence: self.state.command_sequence,
            action,
            suppress_success_sound: action == RecordingAction::Start && self.readiness_policy.sound,
            expected,
            boundary: self.observation_sequence,
            deadline: now.saturating_add(timeout),
        });
        self.state.predicted = expected;
        self.state.pending = self.pending.len();
        if action == RecordingAction::Start {
            self.discard.start_queued(
                TakeToken {
                    generation: self.state.generation,
                    start_sequence: self.state.command_sequence,
                    id: uuid::Uuid::new_v4().to_string(),
                },
                self.file_evidence.clone(),
                confirmed_idle,
                now,
            );
            self.readiness.begin(
                ReadyToken {
                    generation: self.state.generation,
                    start_sequence: self.state.command_sequence,
                    id: uuid::Uuid::new_v4().to_string(),
                },
                self.readiness_policy,
            );
            self.state.speech_readiness = self.readiness.state();
        }
        Ok(CommandReceipt {
            generation: self.state.generation,
            sequence: self.state.command_sequence,
            predicted: expected,
        })
    }
    pub fn expire(&mut self, now: u64) -> Vec<VerificationResult> {
        let mut results = Vec::new();
        while self
            .pending
            .front()
            .is_some_and(|pending| pending.deadline <= now)
        {
            let pending = self.pending.pop_front().unwrap();
            let latest = self.pending.is_empty();
            results.push(VerificationResult {
                discard_operation_id: pending.discard_operation_id,
                generation: self.state.generation,
                sequence: pending.sequence,
                action: pending.action,
                suppress_success_sound: pending.suppress_success_sound,
                status: if latest { "unconfirmed" } else { "superseded" }.into(),
                message: if latest {
                    "verification_unconfirmed".into()
                } else {
                    String::new()
                },
            });
            if latest {
                self.state.predicted = RecordingState::Unknown;
                self.state.condition = "verification_unconfirmed".into();
            }
        }
        self.state.pending = self.pending.len();
        for result in &results {
            self.discard.verification(result, now);
        }
        self.withdraw_cancelled_discard();
        self.reconcile_readiness();
        results
    }
    pub fn delivery_failed(&mut self, sequence: u64) -> Vec<VerificationResult> {
        self.cancel_discard("command_delivery_failed");
        if sequence == self.state.command_sequence {
            self.invalidate_speech_readiness();
        }
        let Some(pending) = self
            .pending
            .iter()
            .find(|pending| pending.sequence == sequence)
            .cloned()
        else {
            return Vec::new();
        };
        self.invalidate_speech_readiness();
        self.pending.clear();
        self.state.pending = 0;
        self.state.predicted = RecordingState::Unknown;
        self.state.condition = "command_delivery_failed".into();
        vec![VerificationResult {
            discard_operation_id: pending.discard_operation_id,
            generation: self.state.generation,
            sequence,
            action: pending.action,
            suppress_success_sound: pending.suppress_success_sound,
            status: "failed".into(),
            message: "command_delivery_failed".into(),
        }]
    }
    pub fn channel_failed(&mut self) {
        self.cancel_discard("command_shell_closed");
        self.invalidate_speech_readiness();
        self.capture_available = false;
        self.pending.clear();
        self.state.pending = 0;
        self.state.observer_ready = false;
        self.state.predicted = RecordingState::Unknown;
        self.state.condition = "command_shell_closed".into();
    }
    fn reconcile_readiness(&mut self) {
        self.readiness.reconcile(&self.state);
        self.state.speech_readiness = self.readiness.state();
    }
    pub fn set_readiness_policy(&mut self, policy: ReadinessPolicy) {
        if !policy.sound {
            self.readiness.disable_sound();
        }
        self.readiness_policy = policy;
    }
    pub fn invalidate_speech_readiness(&mut self) {
        self.readiness.cancel();
        self.state.speech_readiness = self.readiness.state();
    }
    pub fn poll_speech_readiness(&mut self, now_ms: u64) -> (bool, Option<ReadyToken>) {
        let previous = self.state.speech_readiness;
        let token = self.readiness.poll(now_ms, &self.state);
        self.state.speech_readiness = self.readiness.state();
        (previous != self.state.speech_readiness, token)
    }
    pub fn claim_ready_cue(&mut self, token: &ReadyToken) -> bool {
        let claimed = self.readiness.claim(token, &self.state);
        self.state.speech_readiness = self.readiness.state();
        claimed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::readiness::ReadinessPolicy;
    fn queued_discard() -> (
        RecordingMachine,
        u64,
        std::sync::Arc<std::sync::atomic::AtomicU8>,
    ) {
        use crate::discard::FileEvidence;
        let mut model = machine();
        model.file_evidence = Some(FileEvidence {
            stream: "watch".into(),
            sequence: 3,
            complete: true,
            received_ms: 0,
            entries: std::sync::Arc::new(std::collections::BTreeMap::new()),
        });
        model.dispatch(RecordingAction::Start, 1, 1000).unwrap();
        model.observe(observation(2, RecordingState::Recording), 100);
        let take = model.discard.snapshot().take.unwrap();
        let receipt = model.discard.request(&take, 101, 1000).unwrap();
        model.discard.poll(700);
        let stop = model
            .dispatch_raw(
                RecordingAction::Stop,
                700,
                1000,
                Some(&receipt.operation_id),
            )
            .unwrap();
        model
            .discard
            .stop_queued(&receipt.operation_id, stop.sequence);
        let ticket = model.discard_write_ticket(stop.sequence).unwrap();
        (model, stop.sequence, ticket)
    }
    #[test]
    fn new_start_after_withdrawn_stop_is_revalidated_without_a_wrong_toggle() {
        let (mut model, sequence, ticket) = queued_discard();
        model.prepare_intent(RecordingAction::Start);
        assert_eq!(model.state.predicted, RecordingState::Recording);
        assert!(!model.commit_discard_write(sequence, &ticket));
        assert!(
            model
                .dispatch_raw(RecordingAction::Start, 701, 1000, None)
                .is_err()
        );
        assert_eq!(model.state.pending, 0);
    }
    #[test]
    fn queue_failure_cannot_restore_cancelled_discard_write() {
        let (mut model, sequence, ticket) = queued_discard();
        model.prepare_intent(RecordingAction::Pause);
        let rollback = model.clone();
        model
            .dispatch_raw(RecordingAction::Pause, 701, 1000, None)
            .unwrap();
        model = rollback;
        assert!(!model.commit_discard_write(sequence, &ticket));
        assert_eq!(model.state.predicted, RecordingState::Recording);
        assert_eq!(
            model.discard.snapshot().phase,
            crate::discard::DiscardPhase::Retained
        );
    }
    #[test]
    fn confirmed_start_arms_readiness_but_resume_does_not() {
        let mut machine = machine();
        machine.dispatch(RecordingAction::Start, 0, 10_000).unwrap();
        assert_eq!(machine.state.speech_readiness, SpeechReadiness::Preparing);
        assert_eq!(machine.poll_speech_readiness(50_000).1, None);
        let results = machine.observe(observation(2, RecordingState::Recording), 100);
        assert_eq!(results[0].action, RecordingAction::Start);
        assert!(results[0].suppress_success_sound);
        assert_eq!(machine.poll_speech_readiness(899).1, None);
        let token = machine.poll_speech_readiness(900).1.unwrap();
        assert_eq!(token.start_sequence, 1);
        assert!(machine.claim_ready_cue(&token));
        assert!(!machine.claim_ready_cue(&token));
        machine
            .dispatch(RecordingAction::Pause, 901, 10_000)
            .unwrap();
        assert_eq!(machine.state.speech_readiness, SpeechReadiness::Inactive);
        machine.observe(observation(3, RecordingState::Paused), 902);
        machine
            .dispatch(RecordingAction::Resume, 903, 10_000)
            .unwrap();
        let results = machine.observe(observation(4, RecordingState::Recording), 904);
        assert!(!results[0].suppress_success_sound);
        assert_eq!(machine.poll_speech_readiness(100_000).1, None);
        assert_eq!(machine.state.speech_readiness, SpeechReadiness::Inactive);
    }
    #[test]
    fn superseded_confirmation_never_arms_readiness() {
        let mut machine = machine();
        for action in [
            RecordingAction::Start,
            RecordingAction::Stop,
            RecordingAction::Start,
        ] {
            machine.dispatch(action, 0, 10_000).unwrap();
        }
        let results = machine.observe(observation(2, RecordingState::Idle), 100);
        assert_eq!(results[0].status, "superseded");
        assert_eq!(machine.poll_speech_readiness(100_000).1, None);
        assert_eq!(machine.state.speech_readiness, SpeechReadiness::Preparing);
        machine.observe(observation(3, RecordingState::Recording), 2000);
        assert_eq!(machine.poll_speech_readiness(2799).1, None);
        assert_eq!(
            machine
                .poll_speech_readiness(2800)
                .1
                .unwrap()
                .start_sequence,
            3
        );
    }
    #[test]
    fn ready_settings_apply_without_reconnect() {
        let mut machine = machine();
        machine.dispatch(RecordingAction::Start, 0, 10_000).unwrap();
        machine.set_readiness_policy(ReadinessPolicy {
            sound: false,
            delay_ms: 500,
        });
        let results = machine.observe(observation(2, RecordingState::Recording), 100);
        assert!(results[0].suppress_success_sound);
        machine.set_readiness_policy(ReadinessPolicy {
            sound: true,
            delay_ms: 1250,
        });
        assert_eq!(machine.poll_speech_readiness(899).1, None);
        assert_eq!(machine.state.speech_readiness, SpeechReadiness::Preparing);
        assert_eq!(machine.poll_speech_readiness(900).1, None);
        assert_eq!(machine.state.speech_readiness, SpeechReadiness::Ready);
        machine
            .dispatch(RecordingAction::Stop, 1000, 10_000)
            .unwrap();
        machine.observe(observation(3, RecordingState::Idle), 1001);
        machine
            .dispatch(RecordingAction::Start, 1002, 10_000)
            .unwrap();
        machine.observe(observation(4, RecordingState::Recording), 1100);
        assert_eq!(machine.poll_speech_readiness(2349).1, None);
        assert_eq!(
            machine
                .poll_speech_readiness(2350)
                .1
                .unwrap()
                .start_sequence,
            3
        );
    }
    #[test]
    fn late_delivery_failure_cancels_a_confirmed_start() {
        let mut machine = machine();
        machine.dispatch(RecordingAction::Start, 0, 10_000).unwrap();
        machine.observe(observation(2, RecordingState::Recording), 100);
        let token = machine.poll_speech_readiness(900).1.unwrap();
        machine.delivery_failed(1);
        assert!(!machine.claim_ready_cue(&token));
        assert_eq!(machine.state.speech_readiness, SpeechReadiness::Inactive);
    }
    #[test]
    fn timeout_channel_failure_and_foreground_loss_cancel_readiness() {
        for boundary in ["timeout", "channel", "foreground", "explicit"] {
            let mut machine = machine();
            machine.dispatch(RecordingAction::Start, 0, 1000).unwrap();
            if boundary != "timeout" {
                machine.observe(observation(2, RecordingState::Recording), 100);
            }
            match boundary {
                "timeout" => {
                    machine.expire(1000);
                }
                "channel" => machine.channel_failed(),
                "explicit" => machine.invalidate_speech_readiness(),
                _ => {
                    let mut lost = observation(3, RecordingState::Recording);
                    lost.foreground = false;
                    machine.observe(lost, 200);
                }
            }
            assert_eq!(machine.poll_speech_readiness(100_000).1, None);
            assert_eq!(
                machine.state.speech_readiness,
                SpeechReadiness::Inactive,
                "{boundary}"
            );
        }
    }
    fn observation(seq: u64, state: RecordingState) -> Observation {
        Observation {
            version: 1,
            session: "test-session".into(),
            seq,
            event_time: seq * 100,
            foreground: true,
            video_mode: true,
            state,
        }
    }
    fn machine() -> RecordingMachine {
        let mut machine = RecordingMachine::new("test-session".into(), 7);
        machine.observe(observation(1, RecordingState::Idle), 0);
        machine
    }

    #[test]
    fn transitions_match_shared_literal_protocol_cases() {
        let cases: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/protocol/recording-transitions.json"
        ))
        .unwrap();
        for case in cases.as_array().unwrap() {
            let state = serde_json::from_value(case["state"].clone()).unwrap();
            let action = serde_json::from_value(case["action"].clone()).unwrap();
            let expected: RecordingState =
                serde_json::from_value(case["expected"].clone()).unwrap();
            assert_eq!(transition(state, action).unwrap(), expected);
        }
        assert!(transition(RecordingState::Paused, RecordingAction::Start).is_err());
        assert!(transition(RecordingState::Unknown, RecordingAction::Stop).is_err());
    }
    #[test]
    fn rapid_commands_do_not_wait_for_verification() {
        let mut machine = machine();
        for action in [
            RecordingAction::Start,
            RecordingAction::Pause,
            RecordingAction::Resume,
            RecordingAction::Stop,
        ] {
            machine.dispatch(action, 0, 10_000).unwrap();
        }
        assert_eq!(machine.state.predicted, RecordingState::Idle);
        assert_eq!(machine.state.pending, 4);
    }
    #[test]
    fn event_snapshots_have_distinct_revisions_without_new_commands() {
        let mut machine = machine();
        let older = machine.snapshot_for_event();
        let newer = machine.snapshot_for_event();
        assert!(newer.revision > older.revision);
        assert_eq!(newer.command_sequence, older.command_sequence);
        assert_eq!(machine.state.revision, newer.revision);
    }
    #[test]
    fn old_observation_never_rewinds_new_intent() {
        let mut machine = machine();
        machine.dispatch(RecordingAction::Start, 0, 10_000).unwrap();
        machine.dispatch(RecordingAction::Pause, 1, 10_000).unwrap();
        machine.observe(observation(2, RecordingState::Recording), 0);
        assert_eq!(machine.state.predicted, RecordingState::Paused);
        machine.observe(observation(1, RecordingState::Idle), 0);
        assert_eq!(machine.state.observed, RecordingState::Recording);
    }
    #[test]
    fn wrong_session_and_old_timestamps_are_rejected() {
        let mut machine = machine();
        let mut wrong = observation(2, RecordingState::Recording);
        wrong.session = "replaced".into();
        machine.observe(wrong, 0);
        let mut stale = observation(3, RecordingState::Paused);
        stale.event_time = 0;
        machine.observe(stale, 0);
        assert_eq!(machine.state.observed, RecordingState::Idle);
    }
    #[test]
    fn final_coalesced_state_does_not_invent_intermediate_failures() {
        let mut machine = machine();
        machine.dispatch(RecordingAction::Start, 0, 10_000).unwrap();
        machine.dispatch(RecordingAction::Stop, 1, 10_000).unwrap();
        let results = machine.observe(observation(2, RecordingState::Idle), 0);
        assert_eq!(
            results
                .iter()
                .map(|result| result.status.as_str())
                .collect::<Vec<_>>(),
            ["superseded", "confirmed"]
        );
        assert_eq!(machine.state.pending, 0);
    }
    #[test]
    fn delayed_ordered_states_cannot_confirm_resume_from_start_evidence() {
        let mut machine = machine();
        for action in [
            RecordingAction::Start,
            RecordingAction::Pause,
            RecordingAction::Resume,
        ] {
            machine.dispatch(action, 0, 10_000).unwrap();
        }
        let first = machine.observe(observation(2, RecordingState::Recording), 0);
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].sequence, 1);
        assert_eq!(machine.state.pending, 2);
        machine.observe(observation(3, RecordingState::Paused), 0);
        assert_eq!(machine.state.predicted, RecordingState::Recording);
        assert_eq!(machine.state.pending, 1);
        let last = machine.observe(observation(4, RecordingState::Recording), 0);
        assert_eq!(last[0].sequence, 3);
        assert_eq!(machine.state.pending, 0);
    }
    #[test]
    fn ambiguous_coalesced_recording_is_not_proof_of_resume() {
        let mut machine = machine();
        for action in [
            RecordingAction::Start,
            RecordingAction::Pause,
            RecordingAction::Resume,
        ] {
            machine.dispatch(action, 0, 1000).unwrap();
        }
        machine.observe(observation(2, RecordingState::Recording), 0);
        assert_eq!(machine.state.predicted, RecordingState::Recording);
        let results = machine.expire(1000);
        assert_eq!(results.last().unwrap().sequence, 3);
        assert_eq!(results.last().unwrap().status, "unconfirmed");
    }
    #[test]
    fn expired_missing_evidence_is_unconfirmed_not_proven_failure() {
        let mut machine = machine();
        machine.dispatch(RecordingAction::Start, 10, 1000).unwrap();
        assert!(machine.expire(1009).is_empty());
        let results = machine.expire(1010);
        assert_eq!(results[0].status, "unconfirmed");
        assert_eq!(machine.state.pending, 0);
    }
    #[test]
    fn foreground_loss_removes_eligibility_without_retrying() {
        let mut machine = machine();
        let mut lost = observation(2, RecordingState::Unknown);
        lost.foreground = false;
        machine.observe(lost, 0);
        assert!(machine.dispatch(RecordingAction::Start, 0, 1000).is_err());
        assert_eq!(machine.state.command_sequence, 0);
    }
}
