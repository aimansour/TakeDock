use crate::readiness::ReadyToken;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Success,
    Failure,
    SpeechReady,
}
pub type PlayTone = Arc<dyn Fn(Tone) -> Result<(), String> + Send + Sync>;
pub type ClaimReady = Arc<dyn Fn(&ReadyToken) -> bool + Send + Sync>;
pub type AudioFailure = Arc<dyn Fn(Option<ReadyToken>, String) + Send + Sync>;

#[derive(Default)]
struct Queue {
    stopped: bool,
    ready: Option<ReadyToken>,
    newest_ready: Option<(u64, u64)>,
    failures: usize,
    success: bool,
}
struct Shared {
    queue: Mutex<Queue>,
    wake: Condvar,
}
pub struct AudioDispatcher {
    shared: Arc<Shared>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl AudioDispatcher {
    pub fn start(play: PlayTone, claim: ClaimReady, failed: AudioFailure) -> Self {
        let shared = Arc::new(Shared {
            queue: Mutex::new(Queue::default()),
            wake: Condvar::new(),
        });
        let output = shared.clone();
        let worker = thread::spawn(move || {
            let mut feedback_failure_reported = false;
            loop {
                let (tone, token) = {
                    let mut queue = output.queue.lock().unwrap();
                    loop {
                        if queue.stopped {
                            return;
                        }
                        if let Some(token) = queue.ready.take() {
                            // Claim is the playback commit. Ordering it with
                            // shutdown prevents a selected-but-uncommitted cue
                            // escaping shutdown. Playback holds neither lock.
                            if claim(&token) {
                                break (Tone::SpeechReady, Some(token));
                            }
                        } else if queue.failures > 0 {
                            queue.failures -= 1;
                            break (Tone::Failure, None);
                        } else if queue.success {
                            queue.success = false;
                            break (Tone::Success, None);
                        } else {
                            queue = output.wake.wait(queue).unwrap();
                        }
                    }
                };
                match play(tone) {
                    Ok(()) => feedback_failure_reported = false,
                    Err(error) => {
                        let generic = token.is_none();
                        if !generic || !feedback_failure_reported {
                            failed(token, error);
                        }
                        if generic {
                            feedback_failure_reported = true;
                        }
                    }
                }
            }
        });
        Self {
            shared,
            worker: Mutex::new(Some(worker)),
        }
    }
    pub fn feedback(&self, success: bool) {
        let mut queue = self.shared.queue.lock().unwrap();
        if queue.stopped {
            return;
        }
        if success {
            queue.success = true;
        } else {
            queue.failures = (queue.failures + 1).min(16);
        }
        self.shared.wake.notify_one();
    }
    pub fn ready(&self, token: ReadyToken) {
        let mut queue = self.shared.queue.lock().unwrap();
        let key = (token.generation, token.start_sequence);
        if queue.stopped || queue.newest_ready.is_some_and(|newest| key <= newest) {
            return;
        }
        queue.newest_ready = Some(key);
        queue.ready = Some(token);
        self.shared.wake.notify_one();
    }
    pub fn shutdown(&self) {
        {
            let mut queue = self.shared.queue.lock().unwrap();
            queue.stopped = true;
            queue.ready = None;
            queue.failures = 0;
            queue.success = false;
            self.shared.wake.notify_all();
        }
        let worker = self.worker.lock().unwrap().take();
        if let Some(worker) = worker
            && worker.thread().id() != thread::current().id()
        {
            let _ = worker.join();
        }
    }
}
impl Drop for AudioDispatcher {
    fn drop(&mut self) {
        self.shutdown();
    }
}

pub fn play_system_tone(tone: Tone) -> Result<(), String> {
    #[cfg(windows)]
    {
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn Beep(frequency: u32, duration: u32) -> i32;
        }
        let notes: &[(u32, u32)] = match tone {
            Tone::Success => &[(880, 60), (1175, 90)],
            Tone::Failure => &[(330, 110), (220, 160)],
            Tone::SpeechReady => &[(1100, 100)],
        };
        for &(frequency, duration) in notes {
            if unsafe { Beep(frequency, duration) } == 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = tone;
        Err("sound_output_unavailable".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{model::*, readiness::ReadinessPolicy, recording::RecordingMachine};
    use std::{
        sync::{
            atomic::{AtomicBool, Ordering},
            mpsc::{self, Receiver, Sender},
        },
        time::{Duration, Instant},
    };
    const WAIT: Duration = Duration::from_secs(2);

    fn recv<T>(receiver: &Receiver<T>) -> T {
        receiver
            .recv_timeout(WAIT)
            .expect("audio worker did not progress")
    }
    fn observation(seq: u64, state: RecordingState) -> Observation {
        Observation {
            version: 1,
            session: "audio-test".into(),
            seq,
            event_time: seq * 100,
            foreground: true,
            video_mode: true,
            state,
        }
    }
    fn prepared() -> (Arc<Mutex<RecordingMachine>>, ReadyToken) {
        let mut model = RecordingMachine::new("audio-test".into(), 7);
        model.observe(observation(1, RecordingState::Idle), 0);
        model.dispatch(RecordingAction::Start, 0, 10_000).unwrap();
        model.observe(observation(2, RecordingState::Recording), 100);
        let token = model.poll_speech_readiness(900).1.unwrap();
        (Arc::new(Mutex::new(model)), token)
    }
    fn claimer(model: Arc<Mutex<RecordingMachine>>) -> ClaimReady {
        Arc::new(move |token| model.lock().unwrap().claim_ready_cue(token))
    }
    struct HeldPlayer {
        play: PlayTone,
        played: Receiver<Tone>,
        release: Sender<()>,
    }
    fn held_player() -> HeldPlayer {
        let (played, receiver) = mpsc::channel();
        let (release, held) = mpsc::channel();
        let held = Mutex::new(held);
        let first = AtomicBool::new(true);
        HeldPlayer {
            play: Arc::new(move |tone| {
                played.send(tone).unwrap();
                if first.swap(false, Ordering::SeqCst) {
                    held.lock()
                        .unwrap()
                        .recv_timeout(WAIT)
                        .map_err(|_| "test_tone_not_released".to_owned())?;
                }
                Ok(())
            }),
            played: receiver,
            release,
        }
    }
    fn no_failure() -> AudioFailure {
        Arc::new(|_, error| panic!("unexpected audio failure: {error}"))
    }

    #[test]
    fn duplicate_ready_events_play_only_once() {
        let (_, token) = prepared();
        let (tx, rx) = mpsc::channel();
        let audio = AudioDispatcher::start(
            Arc::new(move |tone| {
                tx.send(tone).unwrap();
                Ok(())
            }),
            Arc::new(|_| true),
            no_failure(),
        );
        audio.ready(token.clone());
        assert_eq!(recv(&rx), Tone::SpeechReady);
        for _ in 0..100 {
            audio.ready(token.clone());
        }
        audio.feedback(true);
        assert_eq!(recv(&rx), Tone::Success);
        audio.shutdown();
        assert!(rx.try_recv().is_err());
    }
    #[test]
    fn older_ready_event_cannot_replace_newer_pending_cue() {
        let (model, old) = prepared();
        let newest = {
            let mut model = model.lock().unwrap();
            model.dispatch(RecordingAction::Stop, 1000, 10_000).unwrap();
            model.observe(observation(3, RecordingState::Idle), 1001);
            model
                .dispatch(RecordingAction::Start, 1002, 10_000)
                .unwrap();
            model.observe(observation(4, RecordingState::Recording), 1100);
            model.poll_speech_readiness(1900).1.unwrap()
        };
        let held = held_player();
        let audio = AudioDispatcher::start(held.play, claimer(model), no_failure());
        audio.feedback(false);
        assert_eq!(recv(&held.played), Tone::Failure);
        audio.ready(newest);
        audio.ready(old);
        audio.ready(ReadyToken {
            generation: 6,
            start_sequence: 10_000,
            id: "retired".into(),
        });
        audio.feedback(true);
        held.release.send(()).unwrap();
        assert_eq!(recv(&held.played), Tone::SpeechReady);
        assert_eq!(recv(&held.played), Tone::Success);
        audio.shutdown();
        assert!(held.played.try_recv().is_err());
    }
    #[test]
    fn cancellation_or_disable_reenable_before_commit_skips_the_pending_cue() {
        for boundary in ["cancel", "later_command", "disable_reenable"] {
            let (model, token) = prepared();
            let held = held_player();
            let audio = AudioDispatcher::start(held.play, claimer(model.clone()), no_failure());
            audio.feedback(false);
            assert_eq!(recv(&held.played), Tone::Failure);
            audio.ready(token);
            {
                let mut model = model.lock().unwrap();
                match boundary {
                    "cancel" => model.invalidate_speech_readiness(),
                    "later_command" => {
                        model.dispatch(RecordingAction::Stop, 1000, 10_000).unwrap();
                    }
                    _ => {
                        model.set_readiness_policy(ReadinessPolicy {
                            sound: false,
                            delay_ms: 500,
                        });
                        model.set_readiness_policy(ReadinessPolicy {
                            sound: true,
                            delay_ms: 500,
                        });
                    }
                }
            }
            audio.feedback(true);
            held.release.send(()).unwrap();
            // This success proves the worker passed the cancelled ready slot;
            // shutting down immediately would make this a false positive.
            assert_eq!(recv(&held.played), Tone::Success, "{boundary}");
            audio.shutdown();
            assert!(held.played.try_recv().is_err());
        }
    }
    #[test]
    fn ready_precedes_queued_failure_and_optional_success() {
        let (model, token) = prepared();
        let held = held_player();
        let audio = AudioDispatcher::start(held.play, claimer(model), no_failure());
        audio.feedback(false);
        assert_eq!(recv(&held.played), Tone::Failure);
        audio.feedback(true);
        audio.feedback(false);
        audio.ready(token);
        held.release.send(()).unwrap();
        assert_eq!(recv(&held.played), Tone::SpeechReady);
        assert_eq!(recv(&held.played), Tone::Failure);
        assert_eq!(recv(&held.played), Tone::Success);
        audio.shutdown();
    }
    #[test]
    fn feedback_queue_is_bounded_and_optional_success_is_coalesced() {
        let held = held_player();
        let audio = AudioDispatcher::start(held.play, Arc::new(|_| false), no_failure());
        audio.feedback(false);
        assert_eq!(recv(&held.played), Tone::Failure);
        for _ in 0..100 {
            audio.feedback(false);
            audio.feedback(true);
        }
        held.release.send(()).unwrap();
        for _ in 0..16 {
            assert_eq!(recv(&held.played), Tone::Failure);
        }
        assert_eq!(recv(&held.played), Tone::Success);
        audio.shutdown();
        assert!(held.played.try_recv().is_err());
    }
    #[test]
    fn shutdown_clears_unplayed_cues_and_rejects_later_requests() {
        let (model, token) = prepared();
        let held = held_player();
        let audio = Arc::new(AudioDispatcher::start(
            held.play,
            claimer(model),
            no_failure(),
        ));
        audio.feedback(false);
        assert_eq!(recv(&held.played), Tone::Failure);
        audio.ready(token.clone());
        audio.feedback(true);
        let stopping = audio.clone();
        let shutdown = std::thread::spawn(move || stopping.shutdown());
        // Coordinate the real shutdown boundary while the player is held.
        // Only the eventual playback outputs are asserted, not queue layout.
        let deadline = Instant::now() + WAIT;
        let mut queue = audio.shared.queue.lock().unwrap();
        while !queue.stopped {
            let remaining = deadline.saturating_duration_since(Instant::now());
            assert!(!remaining.is_zero(), "shutdown did not invalidate queue");
            queue = audio.shared.wake.wait_timeout(queue, remaining).unwrap().0;
        }
        drop(queue);
        held.release.send(()).unwrap();
        shutdown.join().unwrap();
        audio.ready(token);
        audio.feedback(false);
        audio.shutdown();
        assert!(held.played.try_recv().is_err());
    }
    #[test]
    fn failed_ready_tone_reports_once_without_audio_retry() {
        let (_, token) = prepared();
        let (played, tones) = mpsc::channel();
        let (failed, errors) = mpsc::channel();
        let audio = AudioDispatcher::start(
            Arc::new(move |tone| {
                played.send(tone).unwrap();
                Err("device_error".into())
            }),
            Arc::new(|_| true),
            Arc::new(move |token, error| {
                failed.send((token, error)).unwrap();
            }),
        );
        audio.ready(token.clone());
        assert_eq!(recv(&errors), (Some(token.clone()), "device_error".into()));
        audio.ready(token);
        audio.shutdown();
        assert_eq!(recv(&tones), Tone::SpeechReady);
        assert!(tones.try_recv().is_err());
        assert!(errors.try_recv().is_err());
    }
    #[test]
    fn continuous_feedback_errors_are_reported_again_only_after_recovery() {
        let (played, tones) = mpsc::channel();
        let (failed, errors) = mpsc::channel();
        let audio = AudioDispatcher::start(
            Arc::new(move |tone| {
                played.send(tone).unwrap();
                if tone == Tone::Success {
                    Ok(())
                } else {
                    Err("device_error".into())
                }
            }),
            Arc::new(|_| false),
            Arc::new(move |token, error| {
                failed.send((token, error)).unwrap();
            }),
        );
        audio.feedback(false);
        assert_eq!(recv(&errors), (None, "device_error".into()));
        assert_eq!(recv(&tones), Tone::Failure);
        audio.feedback(false);
        assert_eq!(recv(&tones), Tone::Failure);
        audio.feedback(true);
        assert_eq!(recv(&tones), Tone::Success);
        audio.feedback(false);
        assert_eq!(recv(&tones), Tone::Failure);
        assert_eq!(recv(&errors), (None, "device_error".into()));
        audio.shutdown();
        assert!(errors.try_recv().is_err());
    }
}
