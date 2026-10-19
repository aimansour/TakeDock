use crate::model::{RecordingState, SessionState, SpeechReadiness};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy)]
pub struct ReadinessPolicy {
    pub sound: bool,
    pub delay_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadyToken {
    pub generation: u64,
    pub start_sequence: u64,
    pub id: String,
}

#[derive(Clone)]
struct Candidate {
    token: ReadyToken,
    policy: ReadinessPolicy,
    deadline: Option<u64>,
    emitted: bool,
    claimed: bool,
}

#[derive(Clone)]
pub struct Readiness {
    generation: u64,
    candidate: Option<Candidate>,
    state: SpeechReadiness,
}

impl Readiness {
    pub fn new(generation: u64) -> Self {
        Self {
            generation,
            candidate: None,
            state: SpeechReadiness::Inactive,
        }
    }

    pub fn begin(&mut self, token: ReadyToken, policy: ReadinessPolicy) -> bool {
        if token.generation != self.generation {
            return false;
        }
        self.candidate = Some(Candidate {
            token,
            policy,
            deadline: None,
            emitted: false,
            claimed: false,
        });
        self.state = SpeechReadiness::Preparing;
        true
    }

    pub fn confirm(&mut self, start_sequence: u64, now_ms: u64) {
        if let Some(candidate) = &mut self.candidate
            && candidate.token.start_sequence == start_sequence
            && candidate.deadline.is_none()
        {
            candidate.deadline = Some(now_ms.saturating_add(candidate.policy.delay_ms));
        }
    }

    pub fn reconcile(&mut self, state: &SessionState) {
        let Some(candidate) = &self.candidate else {
            return;
        };
        let current = state.generation == self.generation
            && state.command_sequence == candidate.token.start_sequence
            && state.connected
            && state.observer_ready
            && state.foreground
            && state.video_mode
            && state.predicted == RecordingState::Recording;
        // Before confirmation, idle/unknown observations can still be the
        // camera preparing. Once confirmed, losing recording cancels forever.
        let confirmed_recording = candidate.deadline.is_none()
            || (state.observed == RecordingState::Recording && state.pending == 0);
        if !current || !confirmed_recording {
            self.cancel();
        }
    }

    pub fn poll(&mut self, now_ms: u64, state: &SessionState) -> Option<ReadyToken> {
        self.reconcile(state);
        let candidate = self.candidate.as_mut()?;
        if !candidate
            .deadline
            .is_some_and(|deadline| now_ms >= deadline)
        {
            return None;
        }
        self.state = SpeechReadiness::Ready;
        if candidate.policy.sound && !candidate.emitted {
            candidate.emitted = true;
            return Some(candidate.token.clone());
        }
        None
    }

    pub fn claim(&mut self, token: &ReadyToken, state: &SessionState) -> bool {
        self.reconcile(state);
        if self.state != SpeechReadiness::Ready {
            return false;
        }
        let Some(candidate) = &mut self.candidate else {
            return false;
        };
        if candidate.token != *token
            || !candidate.policy.sound
            || !candidate.emitted
            || candidate.claimed
        {
            return false;
        }
        candidate.claimed = true;
        true
    }

    pub fn disable_sound(&mut self) {
        if let Some(candidate) = &mut self.candidate {
            candidate.policy.sound = false;
        }
    }

    pub fn cancel(&mut self) {
        self.candidate = None;
        self.state = SpeechReadiness::Inactive;
    }

    pub fn state(&self) -> SpeechReadiness {
        self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{RecordingState, SessionState, SpeechReadiness};

    fn session() -> SessionState {
        SessionState {
            generation: 7,
            command_sequence: 1,
            connected: true,
            observer_ready: true,
            foreground: true,
            video_mode: true,
            predicted: RecordingState::Recording,
            observed: RecordingState::Recording,
            ..Default::default()
        }
    }
    fn token() -> ReadyToken {
        ReadyToken {
            generation: 7,
            start_sequence: 1,
            id: "one".into(),
        }
    }
    fn policy() -> ReadinessPolicy {
        ReadinessPolicy {
            sound: true,
            delay_ms: 800,
        }
    }

    #[test]
    fn deadline_begins_at_confirmation_and_cue_is_claimed_once() {
        let session = session();
        let token = token();
        let mut readiness = Readiness::new(7);
        assert!(readiness.begin(token.clone(), policy()));
        assert!(!readiness.claim(&token, &session));
        assert_eq!(readiness.poll(10_000, &session), None);
        readiness.confirm(1, 20_000);
        readiness.confirm(1, 20_400);
        assert_eq!(readiness.poll(20_799, &session), None);
        assert_eq!(readiness.state(), SpeechReadiness::Preparing);
        assert!(!readiness.claim(&token, &session));
        assert_eq!(readiness.poll(20_800, &session), Some(token.clone()));
        assert_eq!(readiness.state(), SpeechReadiness::Ready);
        assert_eq!(readiness.poll(20_801, &session), None);
        assert!(readiness.claim(&token, &session));
        assert!(!readiness.claim(&token, &session));
    }
    #[test]
    fn wrong_generation_and_confirmation_cannot_replace_or_arm_current_start() {
        let mut readiness = Readiness::new(7);
        readiness.begin(token(), policy());
        assert!(!readiness.begin(
            ReadyToken {
                generation: 8,
                ..token()
            },
            policy()
        ));
        readiness.confirm(2, 10_000);
        assert_eq!(readiness.poll(20_000, &session()), None);
        assert_eq!(readiness.state(), SpeechReadiness::Preparing);
        readiness.confirm(1, 20_000);
        assert_eq!(readiness.poll(20_800, &session()), Some(token()));
        assert!(!readiness.claim(
            &ReadyToken {
                id: "wrong".into(),
                ..token()
            },
            &session()
        ));
        assert!(readiness.claim(&token(), &session()));
    }
    #[test]
    fn cancellation_and_lost_eligibility_prevent_queued_playback() {
        let mut cases = Vec::new();
        let mut state = session();
        state.generation = 8;
        cases.push(state);
        let mut state = session();
        state.command_sequence = 2;
        cases.push(state);
        let mut state = session();
        state.connected = false;
        cases.push(state);
        let mut state = session();
        state.observer_ready = false;
        cases.push(state);
        let mut state = session();
        state.foreground = false;
        cases.push(state);
        let mut state = session();
        state.video_mode = false;
        cases.push(state);
        let mut state = session();
        state.predicted = RecordingState::Idle;
        cases.push(state);
        let mut state = session();
        state.predicted = RecordingState::Paused;
        cases.push(state);
        let mut state = session();
        state.observed = RecordingState::Idle;
        cases.push(state);
        let mut state = session();
        state.observed = RecordingState::Paused;
        cases.push(state);
        let mut state = session();
        state.pending = 1;
        cases.push(state);
        for state in cases {
            let mut readiness = Readiness::new(7);
            readiness.begin(token(), policy());
            readiness.confirm(1, 20_000);
            assert_eq!(readiness.poll(20_800, &session()), Some(token()));
            assert!(!readiness.claim(&token(), &state), "{state:?}");
            assert_eq!(readiness.state(), SpeechReadiness::Inactive);
            assert!(!readiness.claim(&token(), &session()));
        }
        let mut readiness = Readiness::new(7);
        readiness.begin(token(), policy());
        readiness.confirm(1, 20_000);
        readiness.cancel();
        assert_eq!(readiness.poll(30_000, &session()), None);
        assert_eq!(readiness.state(), SpeechReadiness::Inactive);
    }
    #[test]
    fn unknown_before_confirmation_may_recover() {
        let mut state = session();
        state.observed = RecordingState::Unknown;
        state.pending = 1;
        let mut readiness = Readiness::new(7);
        readiness.begin(token(), policy());
        assert_eq!(readiness.poll(10_000, &state), None);
        assert_eq!(readiness.state(), SpeechReadiness::Preparing);
        readiness.confirm(1, 20_000);
        assert_eq!(readiness.poll(20_800, &session()), Some(token()));
    }
    #[test]
    fn unknown_after_confirmation_cancels_permanently() {
        let mut readiness = Readiness::new(7);
        readiness.begin(token(), policy());
        readiness.confirm(1, 20_000);
        let state = SessionState {
            observed: RecordingState::Unknown,
            ..session()
        };
        readiness.reconcile(&state);
        readiness.confirm(1, 20_100);
        assert_eq!(readiness.poll(30_000, &session()), None);
        assert_eq!(readiness.state(), SpeechReadiness::Inactive);
        assert!(!readiness.claim(&token(), &session()));
    }
    #[test]
    fn disable_then_reenable_does_not_revive_current_cue() {
        for disable_after_enqueue in [false, true] {
            let policy = policy();
            let mut readiness = Readiness::new(7);
            readiness.begin(token(), policy);
            readiness.confirm(1, 20_000);
            if disable_after_enqueue {
                assert_eq!(readiness.poll(20_800, &session()), Some(token()));
            }
            readiness.disable_sound();
            assert_eq!(readiness.poll(30_000, &session()), None);
            assert_eq!(readiness.state(), SpeechReadiness::Ready);
            assert!(!readiness.claim(&token(), &session()));
            let next = ReadyToken {
                start_sequence: 2,
                id: "two".into(),
                ..token()
            };
            let state = SessionState {
                command_sequence: 2,
                ..session()
            };
            readiness.begin(next.clone(), policy);
            readiness.confirm(2, 31_000);
            assert_eq!(readiness.poll(31_800, &state), Some(next.clone()));
            assert!(readiness.claim(&next, &state));
            assert!(!readiness.claim(&token(), &state));
        }
    }
    #[test]
    fn sound_off_starts_still_become_visually_ready() {
        let mut readiness = Readiness::new(7);
        readiness.begin(
            token(),
            ReadinessPolicy {
                sound: false,
                delay_ms: 800,
            },
        );
        readiness.confirm(1, 20_000);
        assert_eq!(readiness.poll(20_799, &session()), None);
        assert_eq!(readiness.state(), SpeechReadiness::Preparing);
        assert_eq!(readiness.poll(20_800, &session()), None);
        assert_eq!(readiness.state(), SpeechReadiness::Ready);
        assert!(!readiness.claim(&token(), &session()));
    }
    #[test]
    fn guard_boundaries_and_clock_saturation_never_fire_early() {
        for (confirmed, delay, deadline) in [
            (100, 500, 600),
            (100, 5000, 5100),
            (u64::MAX - 100, 800, u64::MAX),
        ] {
            let mut readiness = Readiness::new(7);
            readiness.begin(
                token(),
                ReadinessPolicy {
                    sound: true,
                    delay_ms: delay,
                },
            );
            readiness.confirm(1, confirmed);
            assert_eq!(readiness.poll(deadline - 1, &session()), None);
            assert_eq!(readiness.poll(deadline, &session()), Some(token()));
        }
    }
}
