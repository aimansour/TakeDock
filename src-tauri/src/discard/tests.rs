use super::*;
use crate::model::{RecordingAction, VerificationResult};
use std::{collections::BTreeMap, sync::Arc};
use takedock_files::{
    Entry, Identity,
    watch::{ManifestEntry, WatchBody, WatchFrame},
};

fn token(sequence: u64) -> TakeToken {
    TakeToken {
        generation: 7,
        start_sequence: sequence,
        id: format!("take-{sequence}"),
    }
}
fn evidence() -> FileEvidence {
    FileEvidence {
        stream: "watch-7".into(),
        sequence: 3,
        complete: true,
        received_ms: 0,
        entries: Arc::new(BTreeMap::new()),
    }
}
fn result(sequence: u64, action: RecordingAction, status: &str) -> VerificationResult {
    VerificationResult {
        discard_operation_id: None,
        generation: 7,
        sequence,
        action,
        status: status.into(),
        message: String::new(),
        suppress_success_sound: false,
    }
}
fn identity(inode: u64, size: u64) -> Identity {
    Identity {
        device: 1,
        inode,
        size,
        modified_ns: size as u128 * 1_000_000,
        changed_ns: size as u128 * 1_000_000,
    }
}
fn frame(sequence: u64, body: WatchBody) -> WatchFrame {
    WatchFrame {
        version: 1,
        stream: "watch-7".into(),
        sequence,
        body,
    }
}
fn created(name: &str, inode: u64, sequence: u64) -> WatchFrame {
    frame(
        sequence,
        WatchBody::Created {
            entry: ManifestEntry {
                name: name.into(),
                identity: identity(inode, 0),
            },
        },
    )
}
fn managed_take() -> (DiscardMachine, TakeToken) {
    let mut machine = DiscardMachine::new(7);
    let take = token(1);
    machine.start_queued(take.clone(), Some(evidence()), true, 1);
    machine.verification(&result(1, RecordingAction::Start, "confirmed"), 100);
    (machine, take)
}

fn renamed(from: &str, name: &str, inode: u64, sequence: u64) -> WatchFrame {
    serde_json::from_value(serde_json::json!({"version":1,"stream":"watch-7","sequence":sequence,
        "body":{"type":"renamed","from":from,"entry":{"name":name,"identity":serde_json::to_string(&identity(inode,100)).unwrap()}}})).unwrap()
}

#[test]
fn finalized_pending_rename_preserves_take_and_restarts_exact_inspections() {
    let (mut machine, operation) = finalizing();
    assert!(
        matches!(machine.poll(900).as_slice(), [DiscardEffect::Inspect { name, .. }] if name == "take.mp4")
    );
    machine.file_frame(&renamed("take.mp4", "final.mp4", 9, 5), 901);
    machine.inspected(&operation, "take.mp4", Err("old_name_gone".into()), 902);
    assert_eq!(machine.snapshot().phase, DiscardPhase::Finalizing);
    assert!(
        matches!(machine.poll(902).as_slice(), [DiscardEffect::Inspect { name, .. }] if name == "final.mp4")
    );
    let mut final_entry = entry();
    final_entry.name = "final.mp4".into();
    machine.inspected(&operation, "final.mp4", Ok(final_entry.clone()), 903);
    assert!(machine.poll(1152).is_empty());
    assert!(
        matches!(machine.poll(1153).as_slice(), [DiscardEffect::Inspect { name, .. }] if name == "final.mp4")
    );
    machine.inspected(&operation, "final.mp4", Ok(final_entry), 1154);
    assert!(
        matches!(machine.poll(1154).as_slice(), [DiscardEffect::Delete { video, .. }] if video.name == "final.mp4")
    );
}

#[test]
fn rename_cannot_adopt_a_replacement_preexisting_target_or_untracked_source() {
    for (from, target, inode) in [
        ("take.mp4", "final.mp4", 10),
        ("other.mp4", "final.mp4", 9),
        ("take.mp4", "old.mp4", 9),
        ("take.mp4", "final.txt", 9),
    ] {
        let mut baseline = evidence();
        Arc::make_mut(&mut baseline.entries).insert("old.mp4".into(), identity(3, 100));
        let mut machine = DiscardMachine::new(7);
        machine.start_queued(token(1), Some(baseline), true, 1);
        machine.file_frame(&created("take.mp4", 9, 4), 100);
        machine.file_frame(&renamed(from, target, inode, 5), 101);
        assert!(!machine.snapshot().eligible);
    }
}

#[test]
fn rename_cancels_an_uncommitted_queued_delete_instead_of_authorizing_its_old_name() {
    let (mut machine, operation) = prepared_delete();
    machine.file_frame(&renamed("take.mp4", "final.mp4", 9, 5), 1153);
    assert!(!machine.authorize_delete(&operation));
    assert_eq!(machine.snapshot().phase, DiscardPhase::Retained);
}
fn finalizing() -> (DiscardMachine, String) {
    let (mut machine, take) = managed_take();
    machine.file_frame(&created("take.mp4", 9, 4), 110);
    let receipt = machine.request(&take, 120, 10_000).unwrap();
    assert!(matches!(
        machine.poll(700).as_slice(),
        [DiscardEffect::SendStop { .. }]
    ));
    machine.stop_queued(&receipt.operation_id, 2);
    machine.verification(&result(2, RecordingAction::Stop, "confirmed"), 900);
    (machine, receipt.operation_id)
}
fn entry() -> Entry {
    Entry {
        name: "take.mp4".into(),
        identity: identity(9, 100),
        ready: true,
        duration_ms: Some(100),
    }
}
fn prepared_delete() -> (DiscardMachine, String) {
    let (mut machine, operation) = finalizing();
    assert!(matches!(
        machine.poll(900).as_slice(),
        [DiscardEffect::Inspect { .. }]
    ));
    machine.inspected(&operation, "take.mp4", Ok(entry()), 901);
    assert!(matches!(
        machine.poll(1151).as_slice(),
        [DiscardEffect::Inspect { .. }]
    ));
    machine.inspected(&operation, "take.mp4", Ok(entry()), 1152);
    assert!(matches!(
        machine.poll(1152).as_slice(),
        [DiscardEffect::Delete { .. }]
    ));
    (machine, operation)
}

#[test]
fn handoff_retires_only_matching_take_and_cannot_be_undone_by_prediction_rollback() {
    let (mut machine, operation) = prepared_delete();
    let before = machine.snapshot();
    assert!(!machine.handed_off("unrelated.mp4", &entry().identity));
    assert_eq!(machine.snapshot(), before);
    let mut rollback = machine.clone();
    assert!(machine.handed_off("take.mp4", &entry().identity));
    assert!(!machine.snapshot().eligible);
    assert!(!rollback.snapshot().eligible);
    assert!(!rollback.authorize_delete(&operation));
    assert!(rollback.request(&token(1), 1200, 10_000).is_err());
}

#[test]
fn handoff_of_an_older_take_does_not_reappear_after_a_new_start_rolls_back() {
    let (mut machine, _) = prepared_delete();
    let mut rollback = machine.clone();
    machine.start_queued(token(2), Some(evidence()), true, 1300);
    let before = machine.snapshot();
    assert!(!machine.handed_off("take.mp4", &entry().identity));
    assert_eq!(machine.snapshot(), before);
    assert!(rollback.request(&token(1), 1301, 10_000).is_err());
}

#[test]
fn late_handoff_cannot_retire_a_reused_name_and_inode_with_a_newer_identity() {
    let (mut machine, _) = prepared_delete();
    machine
        .take
        .as_mut()
        .unwrap()
        .candidate
        .as_mut()
        .unwrap()
        .identity
        .changed_ns = 100_000_100;
    let before = machine.snapshot();
    assert!(!machine.handed_off("take.mp4", &entry().identity));
    assert_eq!(machine.snapshot(), before);
}

#[test]
fn discard_stop_is_requested_once_at_the_600ms_boundary() {
    let (mut machine, take) = managed_take();
    machine.request(&take, 101, 10_000).unwrap();
    assert!(machine.poll(699).is_empty());
    assert!(matches!(
        machine.poll(700).as_slice(),
        [DiscardEffect::SendStop { .. }]
    ));
    assert!(machine.poll(701).is_empty());
}

#[test]
fn late_or_stale_baseline_never_adopts_current_take() {
    for baseline in [
        None,
        Some(FileEvidence {
            complete: false,
            ..evidence()
        }),
        Some(evidence()),
    ] {
        let mut machine = DiscardMachine::new(7);
        machine.start_queued(token(1), baseline, true, 3000);
        machine.file_frame(&created("later.mp4", 9, 4), 3001);
        assert!(!machine.snapshot().eligible);
        assert!(machine.request(&token(1), 3002, 10_000).is_err());
    }
    let mut machine = DiscardMachine::new(7);
    machine.start_queued(token(1), Some(evidence()), false, 1);
    assert!(!machine.snapshot().eligible);
}

#[test]
fn unconfirmed_start_expires_without_stop() {
    let mut machine = DiscardMachine::new(7);
    machine.start_queued(token(1), Some(evidence()), true, 1);
    machine.request(&token(1), 2, 1000).unwrap();
    assert!(machine.poll(1001).is_empty());
    assert!(machine.poll(1002).is_empty());
    assert_eq!(machine.snapshot().phase, DiscardPhase::Retained);
}

#[test]
fn duplicate_request_returns_same_operation() {
    let (mut machine, take) = managed_take();
    let first = machine.request(&take, 101, 10_000).unwrap();
    let second = machine.request(&take, 102, 10_000).unwrap();
    assert_eq!(first.operation_id, second.operation_id);
    assert!(machine.request(&token(3), 103, 10_000).is_err());
}

#[test]
fn only_own_confirmed_stop_can_finalize() {
    let (mut machine, take) = managed_take();
    machine.file_frame(&created("take.mp4", 9, 4), 110);
    let receipt = machine.request(&take, 120, 10_000).unwrap();
    machine.poll(700);
    machine.stop_queued(&receipt.operation_id, 2);
    machine.verification(&result(9, RecordingAction::Stop, "confirmed"), 800);
    assert!(machine.poll(800).is_empty());
    let mut stale = result(2, RecordingAction::Stop, "confirmed");
    stale.generation = 6;
    machine.verification(&stale, 801);
    assert!(machine.poll(801).is_empty());
    machine.verification(&result(2, RecordingAction::Stop, "confirmed"), 900);
    assert!(matches!(
        machine.poll(900).as_slice(),
        [DiscardEffect::Inspect { .. }]
    ));
}

#[test]
fn pause_resume_preserves_tracking_but_cancels_pending_discard() {
    let (mut machine, take) = managed_take();
    machine.ordinary_intent(RecordingAction::Pause);
    machine.ordinary_intent(RecordingAction::Resume);
    assert!(machine.snapshot().eligible);
    machine.request(&take, 101, 10_000).unwrap();
    machine.ordinary_intent(RecordingAction::Pause);
    assert!(machine.poll(700).is_empty());
    assert_eq!(machine.snapshot().phase, DiscardPhase::Retained);
}

#[test]
fn multiple_or_replaced_files_are_retained() {
    for (name, inode) in [("other.mp4", 10), ("take.mp4", 10)] {
        let (mut machine, _) = managed_take();
        machine.file_frame(&created("take.mp4", 9, 4), 110);
        machine.file_frame(&created(name, inode, 5), 111);
        assert!(!machine.snapshot().eligible);
    }
    let mut baseline = evidence();
    Arc::make_mut(&mut baseline.entries).insert("take.mp4".into(), identity(9, 100));
    let mut machine = DiscardMachine::new(7);
    machine.start_queued(token(1), Some(baseline), true, 1);
    machine.file_frame(&created("take.mp4", 10, 4), 110);
    assert!(!machine.snapshot().eligible);
}

#[test]
fn watch_sequence_gap_permanently_invalidates_take() {
    let (mut machine, take) = managed_take();
    machine.file_frame(&created("take.mp4", 9, 5), 110);
    machine.file_frame(&frame(6, WatchBody::Heartbeat), 111);
    assert!(!machine.snapshot().eligible);
    assert!(machine.request(&take, 112, 10_000).is_err());
}

#[test]
fn second_inspection_requires_250ms_and_exact_identity() {
    let (mut machine, operation) = finalizing();
    machine.poll(900);
    machine.inspected(&operation, "take.mp4", Ok(entry()), 901);
    assert!(machine.poll(1150).is_empty());
    assert!(matches!(
        machine.poll(1151).as_slice(),
        [DiscardEffect::Inspect { .. }]
    ));
    let mut changed = entry();
    changed.identity.size += 1;
    machine.inspected(&operation, "take.mp4", Ok(changed), 1152);
    assert!(machine.poll(1152).is_empty());
    assert_eq!(machine.snapshot().phase, DiscardPhase::Retained);
}

#[test]
fn finalization_expires_at_10000ms() {
    let (mut machine, _) = finalizing();
    machine.file_frame(&frame(5, WatchBody::Heartbeat), 10_899);
    assert!(matches!(
        machine.poll(10_899).as_slice(),
        [DiscardEffect::Inspect { .. }]
    ));
    assert!(machine.poll(10_900).is_empty());
    assert_eq!(machine.snapshot().phase, DiscardPhase::Retained);
}

#[test]
fn cancelled_authorization_cannot_be_revived() {
    let (mut machine, operation) = prepared_delete();
    let mut rollback = machine.clone();
    machine.cancel("discard_cancelled");
    assert!(!machine.authorize_delete(&operation));
    assert!(!rollback.authorize_delete(&operation));
}

#[test]
fn authorized_delete_result_is_not_rewritten_as_retained() {
    let (mut machine, operation) = prepared_delete();
    assert!(machine.authorize_delete(&operation));
    assert!(!machine.authorize_delete(&operation));
    machine.cancel("disconnected");
    machine.finish_delete(&operation, DeleteOutcome::Unknown("connection_lost".into()));
    assert_eq!(machine.snapshot().phase, DiscardPhase::OutcomeUnknown);
    assert_eq!(machine.drain_results().len(), 1);
    machine.finish_delete(&operation, DeleteOutcome::Deleted);
    assert!(machine.drain_results().is_empty());
}

#[test]
fn own_delete_events_do_not_cancel_an_authorized_operation() {
    let (mut machine, operation) = prepared_delete();
    assert!(machine.authorize_delete(&operation));
    machine.file_frame(
        &frame(
            5,
            WatchBody::Lost {
                code: "file_watch_history_lost".into(),
            },
        ),
        1153,
    );
    machine.finish_delete(&operation, DeleteOutcome::Deleted);
    assert_eq!(machine.snapshot().phase, DiscardPhase::Completed);
    assert_eq!(machine.drain_results().len(), 1);
}

#[test]
fn old_delete_completion_does_not_adopt_or_cancel_a_newer_take() {
    let (mut machine, operation) = prepared_delete();
    assert!(machine.authorize_delete(&operation));
    let mut baseline = evidence();
    baseline.received_ms = 1200;
    machine.start_queued(token(3), Some(baseline), true, 1200);
    machine.finish_delete(&operation, DeleteOutcome::Deleted);
    assert_eq!(machine.snapshot().take, Some(token(3)));
    assert!(machine.snapshot().eligible);
    let results = machine.drain_results();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].operation_id, operation);
}

#[test]
fn success_without_delete_authorization_cannot_complete_an_operation() {
    let (mut machine, operation) = prepared_delete();
    machine.finish_delete(&operation, DeleteOutcome::Deleted);
    assert_eq!(machine.snapshot().phase, DiscardPhase::Deleting);
    assert!(machine.drain_results().is_empty());
}

#[test]
fn stop_verification_deadline_begins_after_the_delayed_stop() {
    let (mut machine, take) = managed_take();
    machine.request(&take, 101, 1000).unwrap();
    machine.poll(700);
    assert!(machine.poll(1101).is_empty());
    assert_eq!(machine.snapshot().phase, DiscardPhase::ConfirmingStop);
    machine.poll(1700);
    assert_eq!(machine.snapshot().phase, DiscardPhase::Retained);
}
