use crate::files::JobKind;
use serde::{Deserialize, Serialize};

mod model;
pub use model::FileResults;
mod service;
pub use service::FileResultStore;

#[derive(Clone, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResultOwner {
    Job,
    Discard,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResultKey {
    pub generation: u64,
    pub owner: ResultOwner,
    pub operation_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileOutcome {
    Completed,
    Failed,
    Cancelled,
    Retained,
    OutcomeUnknown,
    Warning,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResultItem {
    pub name: String,
    pub status: String,
    pub message: String,
    pub requested_name: Option<String>,
    pub destination_name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileNotice {
    pub key: ResultKey,
    pub revision: u64,
    pub kind: JobKind,
    pub outcome: FileOutcome,
    pub code: String,
    pub warnings: Vec<String>,
    pub name: String,
    pub completed: usize,
    pub count: usize,
    pub results: Vec<ResultItem>,
    pub expires_at_ms: Option<u64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FileResultSnapshot {
    pub revision: u64,
    pub now_ms: u64,
    pub notices: Vec<FileNotice>,
    pub history: Vec<FileNotice>,
}

#[cfg(test)]
mod tests;
