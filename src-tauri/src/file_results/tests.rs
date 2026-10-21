use super::*;
use serde_json::{Value, json};

fn model() -> FileResults {
    let mut model = FileResults::default();
    model.receive("session-state", &json!({"generation":7}), 0);
    model
}
pub(super) fn job(id: &str, status: &str) -> Value {
    json!({"generation":7,"id":id,"kind":"copy","status":status,
        "name":"رحلة.mp4","completed":usize::from(status == "completed"),"count":1,
        "message":if status == "failed" {"copy_hash_mismatch"} else {""},
        "results":[{"name":"رحلة.mp4","status":status,"message":""}]})
}
pub(super) fn warning(id: &str, code: &str) -> Value {
    json!({"generation":7,"id":id,"code":code})
}
fn discard(id: &str, status: &str) -> Value {
    json!({"generation":7,"operation_id":id,"status":status,"code":format!("discard_{status}")})
}

#[test]
fn success_deadline_is_first_terminal_plus_10000() {
    let mut model = model();
    assert!(model.receive("file-job", &job("a", "completed"), 50));
    assert_eq!(
        model.snapshot(10_049).notices[0].expires_at_ms,
        Some(10_050)
    );
    let revision = model.snapshot(100).revision;
    assert!(!model.receive("file-job", &job("a", "completed"), 9_000));
    assert!(!model.receive("file-job", &job("a", "running"), 9_001));
    assert_eq!(model.snapshot(9_999).revision, revision);
    assert!(model.snapshot(10_050).notices.is_empty());
    assert_eq!(model.snapshot(10_050).history.len(), 1);
}

#[test]
fn warnings_survive_completed_and_old_ack() {
    let mut model = model();
    model.receive("file-job", &job("a", "completed"), 0);
    let first = model.snapshot(0).notices[0].clone();
    assert!(model.acknowledge(&first.key, first.revision));
    assert!(model.receive(
        "file-index-warning",
        &warning("a", "media_index_pending"),
        5
    ));
    let newer = model.snapshot(20_000).notices[0].clone();
    assert_eq!(newer.outcome, FileOutcome::Warning);
    assert_eq!(newer.revision, first.revision + 1);
    assert_eq!(newer.expires_at_ms, None);
    assert!(!model.acknowledge(&first.key, first.revision));
    assert!(!model.receive("file-job", &job("a", "completed"), 20_001));
    assert!(!model.receive(
        "file-index-warning",
        &warning("a", "media_index_pending"),
        20_002
    ));
    assert_eq!(model.snapshot(30_000).notices.len(), 1);
    assert!(model.acknowledge(&newer.key, newer.revision));
    assert!(!model.acknowledge(&newer.key, newer.revision));
    assert!(model.snapshot(30_000).notices.is_empty());
}

#[test]
fn warning_before_terminal_is_persistent_without_hiding_failure() {
    for status in ["completed", "failed", "cancelled"] {
        let mut model = model();
        model.receive(
            "file-metadata-warning",
            &warning("a", "rename_refresh_pending"),
            0,
        );
        assert!(model.snapshot(0).history.is_empty());
        model.receive("file-job", &job("a", status), 10);
        let notice = model.snapshot(1_000_000).notices[0].clone();
        assert_eq!(
            notice.outcome,
            match status {
                "failed" => FileOutcome::Failed,
                "cancelled" => FileOutcome::Cancelled,
                _ => FileOutcome::Warning,
            }
        );
        assert_eq!(notice.warnings, ["rename_refresh_pending"]);
    }
}

#[test]
fn failures_outlive_success_and_connection_change() {
    let mut model = model();
    model.receive("file-job", &job("failed", "failed"), 0);
    model.receive("file-job", &job("cancelled", "cancelled"), 1);
    model.receive("discard-result", &discard("retained", "retained"), 2);
    model.receive("discard-result", &discard("unknown", "outcome_unknown"), 3);
    model.receive("file-job", &job("ok", "completed"), 4);
    assert_eq!(model.snapshot(10_004).notices.len(), 4);
    model.receive("session-state", &json!({"generation":8}), 20_000);
    assert!(!model.receive("file-job", &job("stale", "failed"), 20_001));
    assert!(!model.receive("session-state", &json!({"generation":6}), 20_002));
    let snapshot = model.snapshot(30_000);
    assert_eq!(snapshot.notices.len(), 4);
    assert_eq!(snapshot.history.len(), 5);
    assert!(model.acknowledge(&snapshot.notices[0].key, snapshot.notices[0].revision));
    assert_eq!(model.snapshot(30_001).notices.len(), 3);
}

#[test]
fn discard_has_one_notice_with_details_and_aliased_warning() {
    let mut model = model();
    let mut generic = job("delete-job", "queued");
    generic["kind"] = json!("delete");
    generic["discard_operation_id"] = json!("op");
    model.receive("file-job", &generic, 0);
    model.receive("discard-result", &discard("op", "completed"), 50);
    let original = model.snapshot(50).notices[0].clone();
    generic["status"] = json!("completed");
    generic["completed"] = json!(1);
    model.receive("file-job", &generic, 1_000);
    let details = model.snapshot(1_000);
    assert_eq!(details.history.len(), 1);
    assert_eq!(details.notices.len(), 1);
    assert_eq!(details.notices[0].key.owner, ResultOwner::Discard);
    assert_eq!(details.notices[0].revision, original.revision);
    assert_eq!(details.notices[0].expires_at_ms, Some(10_050));
    assert_eq!(details.history[0].results[0].name, "رحلة.mp4");
    model.receive(
        "file-index-warning",
        &warning("delete-job", "media_index_pending"),
        2_000,
    );
    let warned = model.snapshot(50_000);
    assert_eq!(warned.notices.len(), 1);
    assert_eq!(warned.notices[0].key.operation_id, "op");
    assert_eq!(warned.notices[0].outcome, FileOutcome::Warning);
    assert_eq!(warned.history.len(), 1);
}

#[test]
fn history_keeps_twenty_plus_unresolved() {
    let mut model = model();
    model.receive("file-job", &job("failure", "failed"), 0);
    for i in 0..25 {
        model.receive("file-job", &job(&format!("ok-{i}"), "completed"), i + 1);
    }
    let snapshot = model.snapshot(30);
    assert_eq!(snapshot.history.len(), 21);
    assert_eq!(snapshot.notices.len(), 26); // history eviction is not early expiry
    let failed = snapshot
        .notices
        .iter()
        .find(|n| n.key.operation_id == "failure")
        .unwrap();
    model.acknowledge(&failed.key, failed.revision);
    assert_eq!(model.snapshot(30).history.len(), 20);
    assert!(model.snapshot(20_000).notices.is_empty());
}

#[test]
fn evicted_duplicate_cannot_revive() {
    let mut model = model();
    for i in 0..150 {
        model.receive("file-job", &job(&format!("ok-{i}"), "completed"), i);
    }
    assert!(!model.receive("file-job", &job("ok-0", "completed"), 50_000));
    assert!(model.snapshot(50_000).notices.is_empty());
    assert_eq!(model.snapshot(50_000).history.len(), 20);
}

#[test]
fn owner_namespace_and_malformed_evidence_cannot_dismiss_another_result() {
    let mut model = model();
    model.receive("file-job", &job("same", "failed"), 0);
    model.receive("discard-result", &discard("same", "retained"), 0);
    let snapshot = model.snapshot(0);
    assert_eq!(snapshot.notices.len(), 2);
    assert!(!model.acknowledge(
        &ResultKey {
            generation: 8,
            ..snapshot.notices[0].key.clone()
        },
        1
    ));
    assert!(!model.receive(
        "file-job",
        &json!({"generation":7,"id":"oops","status":"completed"}),
        1
    ));
    assert!(!model.receive(
        "operation-error",
        &json!({"generation":7,"code":"capture_failure"}),
        1
    ));
    assert_eq!(model.snapshot(0).revision, snapshot.revision);
}

#[test]
fn late_generic_evidence_cannot_rewrite_confirmed_discard_details() {
    let mut model = model();
    model.receive("discard-result", &discard("op", "completed"), 0);
    let mut generic = job("delete-job", "completed");
    generic["kind"] = json!("delete");
    generic["discard_operation_id"] = json!("op");
    model.receive("file-job", &generic, 1);
    for status in ["queued", "running", "failed", "cancelled"] {
        let mut late = generic.clone();
        late["status"] = json!(status);
        late["results"][0]["status"] = json!(status);
        assert!(!model.receive("file-job", &late, 2));
        let snapshot = model.snapshot(2);
        assert_eq!(snapshot.history[0].results[0].status, "completed");
        assert_eq!(snapshot.history[0].outcome, FileOutcome::Completed);
    }
}
