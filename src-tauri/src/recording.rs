use crate::model::*;
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
}
impl RecordingMachine {
    pub fn new(session: String, generation: u64) -> Self {
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
        }
    }
    pub fn observe(&mut self, observation: Observation) -> Vec<VerificationResult> {
        if observation.version != 1
            || observation.session != self.session
            || observation.seq <= self.observation_sequence
            || observation.event_time < self.event_time
        {
            return Vec::new();
        }
        self.observation_sequence = observation.seq;
        self.event_time = observation.event_time;
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
        let final_match = self.pending.back().is_some_and(|pending| {
            pending.boundary < observation.seq && pending.expected == observation.state
        });
        if final_match {
            while let Some(pending) = self.pending.pop_front() {
                let status = if self.pending.is_empty() {
                    "confirmed"
                } else {
                    "superseded"
                };
                results.push(VerificationResult {
                    generation: self.state.generation,
                    sequence: pending.sequence,
                    status: status.into(),
                    message: String::new(),
                });
            }
        } else if let Some(pending) = self.pending.pop_front_if(|pending| {
            pending.boundary < observation.seq && pending.expected == observation.state
        }) {
            results.push(VerificationResult {
                generation: self.state.generation,
                sequence: pending.sequence,
                status: "confirmed".into(),
                message: String::new(),
            });
        }
        self.state.pending = self.pending.len();
        if self.pending.is_empty() {
            self.state.predicted = observation.state;
        }
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
        let expected = transition(self.state.predicted, action)?;
        self.state.command_sequence += 1;
        self.pending.push_back(Pending {
            sequence: self.state.command_sequence,
            expected,
            boundary: self.observation_sequence,
            deadline: now.saturating_add(timeout),
        });
        self.state.predicted = expected;
        self.state.pending = self.pending.len();
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
        results
    }
    pub fn delivery_failed(&mut self, sequence: u64) -> Vec<VerificationResult> {
        if !self
            .pending
            .iter()
            .any(|pending| pending.sequence == sequence)
        {
            return Vec::new();
        }
        self.pending.clear();
        self.state.pending = 0;
        self.state.predicted = RecordingState::Unknown;
        self.state.condition = "command_delivery_failed".into();
        vec![VerificationResult {
            generation: self.state.generation,
            sequence,
            status: "failed".into(),
            message: "command_delivery_failed".into(),
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        machine.observe(observation(1, RecordingState::Idle));
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
    fn old_observation_never_rewinds_new_intent() {
        let mut machine = machine();
        machine.dispatch(RecordingAction::Start, 0, 10_000).unwrap();
        machine.dispatch(RecordingAction::Pause, 1, 10_000).unwrap();
        machine.observe(observation(2, RecordingState::Recording));
        assert_eq!(machine.state.predicted, RecordingState::Paused);
        machine.observe(observation(1, RecordingState::Idle));
        assert_eq!(machine.state.observed, RecordingState::Recording);
    }
    #[test]
    fn wrong_session_and_old_timestamps_are_rejected() {
        let mut machine = machine();
        let mut wrong = observation(2, RecordingState::Recording);
        wrong.session = "replaced".into();
        machine.observe(wrong);
        let mut stale = observation(3, RecordingState::Paused);
        stale.event_time = 0;
        machine.observe(stale);
        assert_eq!(machine.state.observed, RecordingState::Idle);
    }
    #[test]
    fn final_coalesced_state_does_not_invent_intermediate_failures() {
        let mut machine = machine();
        machine.dispatch(RecordingAction::Start, 0, 10_000).unwrap();
        machine.dispatch(RecordingAction::Stop, 1, 10_000).unwrap();
        let results = machine.observe(observation(2, RecordingState::Idle));
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
        machine.observe(lost);
        assert!(machine.dispatch(RecordingAction::Start, 0, 1000).is_err());
        assert_eq!(machine.state.command_sequence, 0);
    }
}
