use crate::model::*;
use crate::readiness::{Readiness, ReadinessPolicy, ReadyToken};
use std::collections::VecDeque;

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
    sequence: u64,
    action: RecordingAction,
    suppress_success_sound: bool,
    expected: RecordingState,
    boundary: u64,
    deadline: u64,
}
#[derive(Clone)]
pub struct RecordingMachine {
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
    pub fn snapshot_for_event(&mut self) -> SessionState {
        self.state.revision += 1;
        self.state.clone()
    }
    pub fn new(session: String, generation: u64) -> Self {
        let defaults = Settings::default();
        Self {
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
            if result.action == RecordingAction::Start && result.status == "confirmed" {
                self.readiness.confirm(result.sequence, now_ms);
            }
        }
        self.reconcile_readiness();
        results
    }
    pub fn dispatch(
        &mut self,
        action: RecordingAction,
        now: u64,
        timeout: u64,
    ) -> Result<CommandReceipt, String> {
        if !self.state.connected
            || !self.state.observer_ready
            || !self.state.foreground
            || !self.state.video_mode
        {
            return Err("camera_not_eligible".into());
        }
        self.invalidate_speech_readiness();
        let expected = transition(self.state.predicted, action)?;
        self.state.command_sequence += 1;
        self.pending.push_back(Pending {
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
        self.reconcile_readiness();
        results
    }
    pub fn delivery_failed(&mut self, sequence: u64) -> Vec<VerificationResult> {
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
            generation: self.state.generation,
            sequence,
            action: pending.action,
            suppress_success_sound: pending.suppress_success_sound,
            status: "failed".into(),
            message: "command_delivery_failed".into(),
        }]
    }
    pub fn channel_failed(&mut self) {
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
