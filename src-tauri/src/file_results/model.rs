use super::*;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Default)]
pub struct FileResults {
    generation: u64,
    revision: u64,
    order: u64,
    seen: HashMap<ResultKey, Seen>,
    history: Vec<FileNotice>,
    pending: HashMap<ResultKey, Pending>,
    aliases: HashMap<ResultKey, ResultKey>,
}
struct Seen {
    summary: FileNotice,
    acknowledged: Option<u64>,
    order: u64,
}
#[derive(Default)]
struct Pending {
    warnings: Vec<String>,
    details: Option<Details>,
}
#[derive(Clone, PartialEq, Eq)]
struct Details {
    name: String,
    completed: usize,
    count: usize,
    results: Vec<ResultItem>,
}
#[derive(Deserialize)]
struct JobInput {
    id: String,
    kind: JobKind,
    status: String,
    name: String,
    completed: usize,
    count: usize,
    message: String,
    results: Vec<ResultItem>,
    discard_operation_id: Option<String>,
}
impl FileResults {
    pub(super) fn revision(&self) -> u64 {
        self.revision
    }
    pub fn receive(&mut self, event: &str, payload: &Value, now_ms: u64) -> bool {
        let Some(generation) = payload["generation"].as_u64() else {
            return false;
        };
        if event == "session-state" {
            if generation <= self.generation {
                return false;
            }
            self.generation = generation;
            self.pending.clear();
            self.aliases.clear();
            self.revision += 1;
            return true;
        }
        if generation != self.generation {
            return false;
        }
        let changed = match event {
            "file-job" => self.job(payload, now_ms),
            "discard-result" => self.discard(payload, now_ms),
            "file-index-warning" | "file-metadata-warning" => {
                match (text(payload, "id"), text(payload, "code")) {
                    (Some(id), Some(code)) => {
                        let key = self.key(ResultOwner::Job, id);
                        let key = self.aliases.get(&key).cloned().unwrap_or(key);
                        self.warning(key, code)
                    }
                    _ => false,
                }
            }
            _ => false,
        };
        if changed {
            self.revision += 1;
            self.prune_history();
        }
        changed
    }
    pub fn acknowledge(&mut self, key: &ResultKey, revision: u64) -> bool {
        let Some(seen) = self.seen.get_mut(key) else {
            return false;
        };
        if seen.summary.revision != revision || seen.acknowledged == Some(revision) {
            return false;
        }
        seen.acknowledged = Some(revision);
        self.revision += 1;
        self.prune_history();
        true
    }
    pub fn snapshot(&self, now_ms: u64) -> FileResultSnapshot {
        let mut eligible = self
            .seen
            .values()
            .filter(|seen| {
                seen.acknowledged != Some(seen.summary.revision)
                    && seen
                        .summary
                        .expires_at_ms
                        .is_none_or(|deadline| now_ms < deadline)
            })
            .collect::<Vec<_>>();
        eligible.sort_by_key(|seen| std::cmp::Reverse(seen.order));
        FileResultSnapshot {
            revision: self.revision,
            now_ms,
            notices: eligible
                .into_iter()
                .map(|seen| {
                    self.history
                        .iter()
                        .find(|notice| notice.key == seen.summary.key)
                        .unwrap_or(&seen.summary)
                        .clone()
                })
                .collect(),
            history: self.history.clone(),
        }
    }
    fn key(&self, owner: ResultOwner, id: &str) -> ResultKey {
        ResultKey {
            generation: self.generation,
            owner,
            operation_id: id.into(),
        }
    }
    fn job(&mut self, payload: &Value, now_ms: u64) -> bool {
        let Ok(job) = serde_json::from_value::<JobInput>(payload.clone()) else {
            return false;
        };
        if job.id.is_empty() {
            return false;
        }
        if let Some(operation) = job.discard_operation_id {
            if operation.is_empty() {
                return false;
            }
            let generic = self.key(ResultOwner::Job, &job.id);
            let key = self.key(ResultOwner::Discard, &operation);
            self.aliases.insert(generic.clone(), key.clone());
            let orphan = self.pending.remove(&generic).unwrap_or_default();
            let mut changed = false;
            for warning in orphan.warnings {
                changed |= self.warning(key.clone(), &warning);
            }
            let details = Details {
                name: job.name,
                count: job.count,
                completed: job.completed,
                results: job.results,
            };
            if let Some(seen) = self.seen.get(&key) {
                let expected = match seen.summary.outcome {
                    FileOutcome::Completed | FileOutcome::Warning => "completed",
                    FileOutcome::Retained => "cancelled",
                    _ => "failed",
                };
                if job.status == expected {
                    changed |= self.enrich(&key, &details);
                }
            } else {
                let pending = self.pending.entry(key).or_default();
                if pending
                    .details
                    .as_ref()
                    .is_none_or(|old| details.completed >= old.completed)
                {
                    pending.details = Some(details);
                }
            }
            return changed;
        }
        let outcome = match job.status.as_str() {
            "completed" => FileOutcome::Completed,
            "failed" => FileOutcome::Failed,
            "cancelled" => FileOutcome::Cancelled,
            _ => return false,
        };
        let key = self.key(ResultOwner::Job, &job.id);
        let code = if job.message.is_empty() {
            job.status
        } else {
            job.message
        };
        self.insert(
            FileNotice {
                key,
                revision: 1,
                kind: job.kind,
                outcome,
                code,
                warnings: vec![],
                name: job.name,
                completed: job.completed,
                count: job.count,
                results: job.results,
                expires_at_ms: None,
            },
            now_ms,
        )
    }
    fn discard(&mut self, payload: &Value, now_ms: u64) -> bool {
        let (Some(id), Some(status), Some(code)) = (
            text(payload, "operation_id"),
            text(payload, "status"),
            text(payload, "code"),
        ) else {
            return false;
        };
        let outcome = match status {
            "completed" => FileOutcome::Completed,
            "retained" => FileOutcome::Retained,
            "outcome_unknown" => FileOutcome::OutcomeUnknown,
            _ => return false,
        };
        let key = self.key(ResultOwner::Discard, id);
        let details = self.pending.get(&key).and_then(|p| p.details.clone());
        self.insert(
            FileNotice {
                key,
                revision: 1,
                kind: JobKind::Delete,
                completed: usize::from(outcome == FileOutcome::Completed),
                count: 1,
                outcome,
                code: code.into(),
                warnings: vec![],
                name: details.as_ref().map(|d| d.name.clone()).unwrap_or_default(),
                results: details.map(|d| d.results).unwrap_or_default(),
                expires_at_ms: None,
            },
            now_ms,
        )
    }
    fn insert(&mut self, mut notice: FileNotice, now_ms: u64) -> bool {
        if self.seen.contains_key(&notice.key) {
            return false;
        }
        notice.warnings = self
            .pending
            .remove(&notice.key)
            .unwrap_or_default()
            .warnings;
        if notice.outcome == FileOutcome::Completed {
            if notice.warnings.is_empty() {
                notice.expires_at_ms = Some(now_ms.saturating_add(10_000));
            } else {
                notice.outcome = FileOutcome::Warning;
            }
        }
        self.order += 1;
        let mut summary = notice.clone();
        summary.results.clear();
        self.seen.insert(
            notice.key.clone(),
            Seen {
                summary,
                acknowledged: None,
                order: self.order,
            },
        );
        self.history.insert(0, notice);
        true
    }
    fn warning(&mut self, key: ResultKey, code: &str) -> bool {
        if let Some(seen) = self.seen.get_mut(&key) {
            if seen.summary.warnings.iter().any(|warning| warning == code) {
                return false;
            }
            seen.summary.warnings.push(code.into());
            seen.summary.revision += 1;
            seen.summary.expires_at_ms = None;
            if seen.summary.outcome == FileOutcome::Completed {
                seen.summary.outcome = FileOutcome::Warning;
            }
            self.refresh_history(&key);
            true
        } else {
            let pending = self.pending.entry(key).or_default();
            if !pending.warnings.iter().any(|warning| warning == code) {
                pending.warnings.push(code.into());
            }
            false
        }
    }
    fn enrich(&mut self, key: &ResultKey, details: &Details) -> bool {
        let seen = self.seen.get_mut(key).unwrap();
        let name = if details.name.is_empty() {
            &seen.summary.name
        } else {
            &details.name
        };
        let name_changed = seen.summary.name != *name;
        seen.summary.name = name.clone();
        let history = self.history.iter_mut().find(|entry| entry.key == *key);
        let details_changed = history
            .as_ref()
            .is_some_and(|entry| !details.results.is_empty() && entry.results != details.results);
        if !name_changed && !details_changed {
            return false;
        }
        if let Some(history) = history {
            history.name = seen.summary.name.clone();
            if !details.results.is_empty() {
                history.results = details.results.clone();
            }
        }
        true
    }
    fn refresh_history(&mut self, key: &ResultKey) {
        let mut summary = self.seen[key].summary.clone();
        if let Some(entry) = self.history.iter_mut().find(|entry| entry.key == *key) {
            summary.results = std::mem::take(&mut entry.results);
            *entry = summary;
        } else {
            self.history.insert(0, summary);
        }
    }
    fn prune_history(&mut self) {
        let mut ordinary = 0;
        self.history.retain(|notice| {
            let seen = &self.seen[&notice.key];
            if notice.outcome != FileOutcome::Completed
                && seen.acknowledged != Some(notice.revision)
            {
                true
            } else {
                ordinary += 1;
                ordinary <= 20
            }
        });
    }
}

fn text<'a>(payload: &'a Value, key: &str) -> Option<&'a str> {
    payload[key].as_str().filter(|value| !value.is_empty())
}
