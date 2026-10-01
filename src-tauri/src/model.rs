use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordingState {
    #[default]
    Unknown,
    Idle,
    Recording,
    Paused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordingAction {
    Start,
    Stop,
    Pause,
    Resume,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub language: String,
    pub destination: String,
    pub adb_path: String,
    pub success_sound: bool,
    pub verification_ms: u64,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            language: "en".into(),
            destination: String::new(),
            adb_path: String::new(),
            success_sound: false,
            verification_ms: 10_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub version: u32,
    pub session: String,
    pub seq: u64,
    pub event_time: u64,
    pub foreground: bool,
    pub video_mode: bool,
    pub state: RecordingState,
}

#[derive(Debug, Clone, Serialize)]
pub struct VerificationResult {
    pub generation: u64,
    pub sequence: u64,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct SessionState {
    pub generation: u64,
    pub revision: u64,
    pub connected: bool,
    pub observer_ready: bool,
    pub foreground: bool,
    pub video_mode: bool,
    pub predicted: RecordingState,
    pub observed: RecordingState,
    pub command_sequence: u64,
    pub pending: usize,
    pub condition: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CommandReceipt {
    pub generation: u64,
    pub sequence: u64,
    pub predicted: RecordingState,
}
