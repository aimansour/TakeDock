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
                    self.jobs.truncate(20);
                }
                match status {
                    "completed" => (format!("job:{id}"), true),
                    "failed" => (format!("job:{id}"), false),
                    _ => return None,
                }
            }
            "verification-result" => {
                let sequence = value["sequence"].as_u64()?;
                match status {
                    "confirmed" => (format!("command:{sequence}"), true),
                    "failed" | "unconfirmed" => (format!("command:{sequence}"), false),
                    _ => return None,
                }
            }
            "operation-error" => (format!("error:{}", value["code"].as_str()?), false),
            _ => return None,
        };
        if self.sounded.contains(&key) {
            return None;
        }
        self.sounded.push_back(key);
        if self.sounded.len() > 128 {
            self.sounded.pop_front();
        }
        Some(success)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
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
