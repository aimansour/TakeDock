use crate::offers::Offers;
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};
use tauri_plugin_updater::Update;
pub struct Updates {
    pub offered: Mutex<Offers<Update>>,
    pub installing: AtomicBool,
}
impl Default for Updates {
    fn default() -> Self {
        Self {
            offered: Mutex::new(Offers::default()),
            installing: AtomicBool::new(false),
        }
    }
}
impl Updates {
    pub fn start_install(&self, token: &str) -> Result<Update, String> {
        if self.installing.swap(true, Ordering::AcqRel) {
            Err("update_in_progress".into())
        } else {
            let result = self.offered.lock().unwrap().take(token);
            if result.is_err() {
                self.installing.store(false, Ordering::Release);
            }
            result
        }
    }
}
#[derive(serde::Serialize)]
pub struct UpdateInfo {
    pub token: String,
    pub version: String,
    pub notes: String,
    pub date: String,
}
