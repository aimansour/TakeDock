use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};
use tauri_plugin_updater::Update;
pub struct Updates {
    pub offered: Mutex<Option<Update>>,
    pub installing: AtomicBool,
}
impl Default for Updates {
    fn default() -> Self {
        Self {
            offered: Mutex::new(None),
            installing: AtomicBool::new(false),
        }
    }
}
impl Updates {
    pub fn start_install(&self) -> Result<(), String> {
        if self.installing.swap(true, Ordering::AcqRel) {
            Err("update_in_progress".into())
        } else {
            Ok(())
        }
    }
}
#[derive(serde::Serialize)]
pub struct UpdateInfo {
    pub version: String,
    pub notes: String,
    pub date: String,
}
