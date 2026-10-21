use serde_json::Value;
use std::collections::VecDeque;

#[derive(Default)]
pub struct Events {
    pub jobs: Vec<Value>,
    generation: u64,
    sounded: VecDeque<String>,
}
impl Events {
    pub fn receive(&mut self, event: &str, value: &Value) -> Option<bool> {
        let generation = value["generation"].as_u64()?;
        if event == "session-state" {
            if generation < self.generation {
                return None;
            }
            if generation != self.generation {
                self.jobs.clear();
                self.sounded.clear();
            }
            self.generation = generation;
            return None;
        }
        if generation != self.generation {
            return None;
        }
        let status = value["status"].as_str().unwrap_or("");
        let (key, success) = match event {
            "file-job" => {
                let id = value["id"].as_str()?;
                if let Some(index) = self.jobs.iter().position(|job| job["id"] == id) {
                    let previous = self.jobs[index]["status"].as_str().unwrap_or("");
                    if (status == "queued" && previous != "queued")
                        || (["completed", "failed", "cancelled"].contains(&previous)
                            && ["queued", "running"].contains(&status))
                    {
                        return None;
                    }
                    self.jobs[index] = value.clone();
                } else {
                    self.jobs.insert(0, value.clone());
                }
                let mut terminal = 0;
                self.jobs.retain(|job| {
                    if matches!(job["status"].as_str(), Some("queued" | "running")) {
                        true
                    } else {
                        terminal += 1;
                        terminal <= 20
                    }
                });
                if value["discard_operation_id"].is_string() {
                    return None;
                }
                match status {
                    "completed" => (format!("job:{id}"), true),
                    "failed" => (format!("job:{id}"), false),
                    _ => return None,
                }
            }
            "verification-result" => {
                if value["discard_operation_id"].is_string() {
                    return None;
                }
                let sequence = value["sequence"].as_u64()?;
                match status {
                    "confirmed" => (format!("command:{sequence}"), true),
                    "failed" | "unconfirmed" => (format!("command:{sequence}"), false),
                    _ => return None,
                }
            }
            "operation-error" => (format!("error:{}", value["code"].as_str()?), false),
            "discard-result" => {
                let operation = value["operation_id"].as_str()?;
                match status {
                    "completed" => (format!("discard:{operation}"), true),
                    "retained" | "outcome_unknown" => (format!("discard:{operation}"), false),
                    _ => return None,
                }
            }
            _ => return None,
        };
        if self.sounded.contains(&key) {
            return None;
        }
        self.sounded.push_back(key);
        if self.sounded.len() > 128 {
            self.sounded.pop_front();
        }
        if event == "verification-result"
            && success
            && value["action"] == "start"
            && value["suppress_success_sound"] == true
        {
            None
        } else {
            Some(success)
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn active_jobs_survive_twenty_terminals() {
        let mut events = Events::default();
        events.receive("session-state", &json!({"generation":7}));
        events.receive(
            "file-job",
            &json!({"generation":7,"id":"held","status":"running"}),
        );
        for i in 0..25 {
            events.receive(
                "file-job",
                &json!({"generation":7,"id":format!("done-{i}"),"status":"completed"}),
            );
        }
        assert!(events.jobs.iter().any(|job| job["id"] == "held"));
        assert_eq!(events.jobs.len(), 21);
    }
    #[test]
    fn older_session_state_cannot_restore_a_discard_feedback_generation() {
        let mut events = Events::default();
        events.receive("session-state", &json!({"generation":7}));
        events.receive("session-state", &json!({"generation":6}));
        assert_eq!(
            events.receive(
                "discard-result",
                &json!({"generation":6,"operation_id":"old","status":"outcome_unknown"})
            ),
            None
        );
    }
    #[test]
    fn discard_owns_feedback_once_instead_of_stop_and_file_jobs() {
        let mut events = Events::default();
        events.receive("session-state", &json!({"generation":7}));
        assert_eq!(events.receive("verification-result", &json!({"generation":7,"sequence":2,"action":"stop","status":"confirmed","discard_operation_id":"op-1"})), None);
        assert_eq!(events.receive("file-job", &json!({"generation":7,"id":"job-1","status":"completed","discard_operation_id":"op-1"})), None);
        let result = json!({"generation":7,"operation_id":"op-1","status":"completed","code":"discard_completed"});
        assert_eq!(events.receive("discard-result", &result), Some(true));
        assert_eq!(events.receive("discard-result", &result), None);
        events.receive("session-state", &json!({"generation":7}));
        assert_eq!(events.receive("discard-result", &result), None);
        let stale = json!({"generation":6,"operation_id":"old","status":"outcome_unknown"});
        assert_eq!(events.receive("discard-result", &stale), None);
    }
    #[test]
    fn confirmed_start_does_not_play_early_success_feedback() {
        let mut events = Events::default();
        events.receive("session-state", &json!({"generation":7}));
        let mut result = json!({"generation":7,"sequence":1,"action":"start",
            "status":"confirmed","suppress_success_sound":true});
        assert_eq!(events.receive("verification-result", &result), None);
        result["suppress_success_sound"] = json!(false);
        assert_eq!(events.receive("verification-result", &result), None);
        for (sequence, action, status, suppressed, expected) in [
            (2, "start", "confirmed", false, Some(true)),
            (3, "pause", "confirmed", true, Some(true)),
            (4, "start", "failed", true, Some(false)),
            (5, "start", "unconfirmed", true, Some(false)),
        ] {
            let result = json!({"generation":7,"sequence":sequence,"action":action,
                "status":status,"suppress_success_sound":suppressed});
            assert_eq!(events.receive("verification-result", &result), expected);
            assert_eq!(events.receive("verification-result", &result), None);
        }
    }
    #[test]
    fn shared_results_sound_once_and_ignore_superseded_or_stale_evidence() {
        let mut events = Events::default();
        events.receive("session-state", &json!({"generation":3}));
        let result = json!({"generation":3,"sequence":2,"status":"confirmed"});
        assert_eq!(events.receive("verification-result", &result), Some(true));
        assert_eq!(events.receive("verification-result", &result), None);
        assert_eq!(
            events.receive(
                "verification-result",
                &json!({"generation":3,"sequence":3,"status":"superseded"})
            ),
            None
        );
        assert_eq!(
            events.receive(
                "verification-result",
                &json!({"generation":2,"sequence":4,"status":"failed"})
            ),
            None
        );
        assert_eq!(
            events.receive(
                "verification-result",
                &json!({"generation":3,"sequence":4,"status":"unconfirmed"})
            ),
            Some(false)
        );
    }
    #[test]
    fn a_new_window_gets_latest_job_progress_and_only_terminal_jobs_sound() {
        let mut events = Events::default();
        events.receive("session-state", &json!({"generation":3}));
        for status in ["queued", "running", "completed"] {
            let job = json!({"generation":3,"id":"one","status":status,"bytes":20,"total":20});
            assert_eq!(
                events.receive("file-job", &job),
                if status == "completed" {
                    Some(true)
                } else {
                    None
                }
            );
        }
        assert_eq!(events.jobs.len(), 1);
        assert_eq!(events.jobs[0]["status"], "completed");
        events.receive("session-state", &json!({"generation":4}));
        assert!(events.jobs.is_empty());
        assert_eq!(
            events.receive(
                "file-job",
                &json!({"generation":3,"id":"old","status":"failed"})
            ),
            None
        );
        assert!(events.jobs.is_empty());
    }
}
