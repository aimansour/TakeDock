use super::*;
use crate::{discard::DeleteOutcome, file_watch::WatchUpdate};

impl Shared {
    pub(super) fn dispatch_capture(
        &self,
        action: RecordingAction,
        generation: u64,
        operation: Option<&str>,
    ) -> Result<CommandReceipt, String> {
        let active_guard = self.active.lock().unwrap();
        let active = active_guard.as_ref().ok_or("no_device")?;
        if active.lease.generation != generation || !self.current(generation) {
            return Err("session_replaced".into());
        }
        let mut model = self.model.lock().unwrap();
        // Permanent intent cancellation precedes the rollback snapshot.
        if operation.is_none() {
            model.prepare_intent(action);
        }
        let previous = model.clone();
        let receipt = match model.dispatch_raw(
            action,
            self.now(),
            self.settings.verification_ms,
            operation,
        ) {
            Ok(receipt) => receipt,
            Err(error) => {
                drop(model);
                drop(active_guard);
                self.state();
                return Err(error);
            }
        };
        let ticket = model.discard_write_ticket(receipt.sequence);
        if active
            .sender
            .try_send(QueuedCommand {
                sequence: receipt.sequence,
                action,
                ticket,
            })
            .is_err()
        {
            *model = previous;
            model.invalidate_speech_readiness();
            drop(model);
            drop(active_guard);
            self.state();
            return Err("command_queue_unavailable".into());
        }
        if let Some(operation) = operation {
            model.discard.stop_queued(operation, receipt.sequence);
        }
        drop(model);
        drop(active_guard);
        self.state();
        Ok(receipt)
    }
    pub(super) fn request_discard(
        &self,
        take: TakeToken,
        generation: u64,
    ) -> Result<DiscardReceipt, String> {
        let active = self.active.lock().unwrap();
        if !self.current(generation)
            || active
                .as_ref()
                .is_none_or(|active| active.lease.generation != generation)
        {
            return Err("session_replaced".into());
        }
        let mut model = self.model.lock().unwrap();
        if model.discard.snapshot().take.as_ref() != Some(&take) || take.generation != generation {
            return Err("discard_take_replaced".into());
        }
        if model.discard.snapshot().operation_id.is_some() {
            return model
                .discard
                .request(&take, self.now(), self.settings.verification_ms);
        }
        if !model.state.observer_ready || !model.state.foreground || !model.state.video_mode {
            return Err("camera_not_eligible".into());
        }
        if model.state.pending > 0 && model.state.command_sequence != take.start_sequence {
            return Err("discard_capture_pending".into());
        }
        let result = model
            .discard
            .request(&take, self.now(), self.settings.verification_ms);
        if result.is_ok() {
            model.invalidate_speech_readiness();
        }
        drop(model);
        drop(active);
        self.state();
        result
    }
    pub(super) fn poll_discard(&self) {
        let (generation, changed, effects) = {
            let mut model = self.model.lock().unwrap();
            let before = model.discard.snapshot();
            let effects = model.discard.poll(self.now());
            model.withdraw_cancelled_discard();
            (
                model.state.generation,
                before != model.discard.snapshot(),
                effects,
            )
        };
        for effect in effects {
            if self.effects.try_send((generation, effect)).is_err() {
                self.model
                    .lock()
                    .unwrap()
                    .cancel_discard("discard_worker_unavailable");
                self.state();
            }
        }
        if changed {
            self.state();
        }
    }
    fn fail_discard(&self, operation: &str, code: &str) {
        let mut model = self.model.lock().unwrap();
        if model.discard.snapshot().operation_id.as_deref() == Some(operation) {
            model.cancel_discard(code);
        }
        drop(model);
        self.state();
    }
    pub(super) fn run_discard_effects(
        self: &Arc<Self>,
        receiver: mpsc::Receiver<(u64, DiscardEffect)>,
    ) {
        while !self.stopped.load(Ordering::Acquire) {
            let (generation, effect) = match receiver.recv_timeout(Duration::from_millis(25)) {
                Ok(effect) => effect,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(_) => break,
            };
            if !self.current(generation) {
                continue;
            }
            match effect {
                DiscardEffect::SendStop { operation_id } => {
                    if self
                        .dispatch_capture(RecordingAction::Stop, generation, Some(&operation_id))
                        .is_err()
                    {
                        self.fail_discard(&operation_id, "discard_stop_unconfirmed");
                    }
                }
                DiscardEffect::Inspect { operation_id, name } => {
                    let watcher = self
                        .watcher
                        .lock()
                        .unwrap()
                        .as_ref()
                        .filter(|(current, _)| *current == generation)
                        .map(|(_, watch)| watch.clone());
                    if watcher
                        .is_none_or(|watcher| watcher.inspect(operation_id.clone(), name).is_err())
                    {
                        self.fail_discard(&operation_id, "discard_inspection_failed");
                    }
                }
                DiscardEffect::Delete {
                    operation_id,
                    video,
                } => {
                    let lease = self
                        .active
                        .lock()
                        .unwrap()
                        .as_ref()
                        .filter(|active| active.lease.generation == generation)
                        .map(|active| active.lease.clone());
                    let Some(lease) = lease else {
                        self.fail_discard(&operation_id, "discard_disconnected");
                        continue;
                    };
                    let Some(services) = &self.services else {
                        self.fail_discard(&operation_id, "discard_worker_unavailable");
                        continue;
                    };
                    let authorizer = Arc::downgrade(self);
                    let preparer = Arc::downgrade(self);
                    let preparation_id = operation_id.clone();
                    let should_prepare = Arc::new(move || {
                        preparer.upgrade().is_some_and(|shared| {
                            shared.current(generation)
                                && shared
                                    .model
                                    .lock()
                                    .unwrap()
                                    .discard
                                    .delete_pending(&preparation_id)
                        })
                    });
                    let authorization_id = operation_id.clone();
                    let authorize = Arc::new(move || {
                        let Some(shared) = authorizer.upgrade() else {
                            return false;
                        };
                        if !shared.current(generation) {
                            return false;
                        }
                        let mut model = shared.model.lock().unwrap();
                        model.discard.poll(shared.now());
                        model.state.generation == generation
                            && model.state.connected
                            && model.state.observer_ready
                            && model.state.foreground
                            && model.state.video_mode
                            && model.state.observed == RecordingState::Idle
                            && model.state.predicted == RecordingState::Idle
                            && model.state.pending == 0
                            && model.discard.authorize_delete(&authorization_id)
                    });
                    let completer = Arc::downgrade(self);
                    let completion_id = operation_id.clone();
                    let complete = Arc::new(move |outcome: DeleteOutcome| {
                        let Some(shared) = completer.upgrade() else {
                            return;
                        };
                        if !shared.current(generation) {
                            return;
                        }
                        shared
                            .model
                            .lock()
                            .unwrap()
                            .discard
                            .finish_delete(&completion_id, outcome);
                        shared.state();
                    });
                    if services
                        .files
                        .start_discard_delete_checked(
                            lease,
                            video,
                            operation_id.clone(),
                            should_prepare,
                            authorize,
                            complete,
                        )
                        .is_err()
                    {
                        self.fail_discard(&operation_id, "discard_worker_unavailable");
                    }
                }
            }
        }
    }
    pub(super) fn run_watch_manager(self: &Arc<Self>) {
        let resources = self.services.as_ref().unwrap().resources.clone();
        while !self.stopped.load(Ordering::Acquire) {
            let (generation, idle) = {
                let model = self.model.lock().unwrap();
                (
                    model.state.generation,
                    model.state.connected
                        && model.state.observer_ready
                        && model.state.observed == RecordingState::Idle
                        && model.state.predicted == RecordingState::Idle
                        && model.state.pending == 0,
                )
            };
            let previous = {
                let mut slot = self.watcher.lock().unwrap();
                if slot.as_ref().is_some_and(|(current, watcher)| {
                    *current != generation || (idle && watcher.is_stopped())
                }) {
                    slot.take()
                } else {
                    None
                }
            };
            if let Some((_, watcher)) = previous {
                watcher.shutdown();
            }
            if idle && self.watcher.lock().unwrap().is_none() {
                let lease = self
                    .active
                    .lock()
                    .unwrap()
                    .as_ref()
                    .filter(|active| active.lease.generation == generation)
                    .map(|active| active.lease.clone());
                if let Some(lease) = lease {
                    let callback_slot = Arc::downgrade(self);
                    let callback = Arc::new(move |update: WatchUpdate| {
                        let Some(shared) = callback_slot.upgrade() else {
                            return;
                        };
                        if !shared.current(generation) {
                            return;
                        }
                        let mut model = shared.model.lock().unwrap();
                        if model.state.generation != generation {
                            return;
                        }
                        match update {
                            WatchUpdate::Frame { frame, evidence } => {
                                model.discard.file_frame(&frame, shared.now());
                                model.file_evidence = evidence;
                            }
                            WatchUpdate::Inspection {
                                operation_id,
                                name,
                                entry,
                            } => model
                                .discard
                                .inspected(&operation_id, &name, entry, shared.now()),
                            WatchUpdate::Lost(_) => {
                                model.file_evidence = None;
                                model.cancel_discard("discard_file_history_lost");
                            }
                        }
                        model.withdraw_cancelled_discard();
                        drop(model);
                        shared.state();
                    });
                    let origin = self.started;
                    let watcher = Arc::new(FileWatch::start(
                        lease,
                        resources.clone(),
                        Arc::new(move || origin.elapsed().as_millis() as u64),
                        callback,
                    ));
                    if self.current(generation) {
                        *self.watcher.lock().unwrap() = Some((generation, watcher));
                    } else {
                        watcher.shutdown();
                    }
                }
            }
            thread::sleep(Duration::from_millis(25));
        }
        let watcher = self.watcher.lock().unwrap().take();
        if let Some((_, watcher)) = watcher {
            watcher.shutdown();
        }
    }
}
