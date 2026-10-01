use crate::updates::{UpdateInfo, Updates};
use crate::{
    files::{FileService, JobKind, Video},
    model::*,
    session::{Engine, EventSink},
    settings,
};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex, RwLock},
};
use tauri::{Emitter, Manager, State};
use tauri_plugin_updater::UpdaterExt;

struct Runtime {
    engine: RwLock<Option<Arc<Engine>>>,
    files: FileService,
    settings: Mutex<Settings>,
    settings_path: PathBuf,
    resources: PathBuf,
    sink: EventSink,
    errors: Mutex<Vec<String>>,
    reconnect: Mutex<()>,
    updates: Updates,
}
impl Runtime {
    fn engine(&self) -> Result<Arc<Engine>, String> {
        self.engine
            .read()
            .unwrap()
            .clone()
            .ok_or("no_device".into())
    }
    fn reconnect(&self) -> Result<(), String> {
        let _guard = self.reconnect.lock().unwrap();
        if let Some(engine) = self.engine.write().unwrap().take() {
            engine.shutdown();
        }
        let engine = Engine::start(
            self.settings.lock().unwrap().clone(),
            self.resources.join("observer.apk"),
            self.sink.clone(),
        )?;
        *self.engine.write().unwrap() = Some(Arc::new(engine));
        Ok(())
    }
    fn shutdown(&self) {
        if let Some(engine) = self.engine.write().unwrap().take() {
            engine.shutdown();
        }
        self.files.shutdown();
    }
}
#[derive(serde::Serialize)]
struct Bootstrap {
    settings: Settings,
    session: SessionState,
    errors: Vec<String>,
}
#[tauri::command]
fn bootstrap(state: State<'_, Arc<Runtime>>) -> Bootstrap {
    Bootstrap {
        settings: state.settings.lock().unwrap().clone(),
        session: state
            .engine()
            .map(|engine| engine.snapshot())
            .unwrap_or_default(),
        errors: state.errors.lock().unwrap().clone(),
    }
}
#[tauri::command]
fn recording_action(
    action: RecordingAction,
    generation: u64,
    state: State<'_, Arc<Runtime>>,
) -> Result<CommandReceipt, String> {
    state.engine()?.dispatch_for(action, generation)
}
#[tauri::command]
async fn list_videos(
    generation: u64,
    state: State<'_, Arc<Runtime>>,
) -> Result<Vec<Video>, String> {
    let runtime = state.inner().clone();
    let lease = runtime.engine()?.lease_for(generation)?;
    tauri::async_runtime::spawn_blocking(move || runtime.files.list(lease))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
fn start_file_job(
    kind: JobKind,
    videos: Vec<Video>,
    generation: u64,
    new_stem: Option<String>,
    state: State<'_, Arc<Runtime>>,
) -> Result<String, String> {
    let lease = state.engine()?.lease_for(generation)?;
    let destination = PathBuf::from(&state.settings.lock().unwrap().destination);
    state
        .files
        .start_job(lease, kind, videos, destination, new_stem)
}
#[tauri::command]
fn cancel_file_job(id: String, state: State<'_, Arc<Runtime>>) -> Result<(), String> {
    state.files.cancel(&id)
}
#[tauri::command]
async fn save_settings(
    settings: Settings,
    state: State<'_, Arc<Runtime>>,
) -> Result<Settings, String> {
    let runtime = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut current = runtime.settings.lock().unwrap();
        settings::save(&runtime.settings_path, &settings)?;
        *current = settings.clone();
        Ok(settings)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn reconnect(state: State<'_, Arc<Runtime>>) -> Result<(), String> {
    let runtime = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || runtime.reconnect())
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
fn play_feedback(success: bool, state: State<'_, Arc<Runtime>>) {
    if success && !state.settings.lock().unwrap().success_sound {
        return;
    }
    #[cfg(windows)]
    std::thread::spawn(move || {
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn Beep(frequency: u32, duration: u32) -> i32;
        }
        let notes = if success {
            [(880, 60), (1175, 90)]
        } else {
            [(330, 110), (220, 160)]
        };
        for (frequency, duration) in notes {
            unsafe {
                Beep(frequency, duration);
            }
        }
    });
}
#[tauri::command]
async fn check_update(
    app: tauri::AppHandle,
    state: State<'_, Arc<Runtime>>,
) -> Result<Option<UpdateInfo>, String> {
    if state
        .updates
        .installing
        .load(std::sync::atomic::Ordering::Acquire)
    {
        return Err("update_in_progress".into());
    }
    let runtime = state.inner().clone();
    let cleanup = runtime.clone();
    let update = app
        .updater_builder()
        .timeout(std::time::Duration::from_secs(15))
        .on_before_exit(move || cleanup.shutdown())
        .build()
        .map_err(|error| error.to_string())?
        .check()
        .await
        .map_err(|error| error.to_string())?;
    let info = update.as_ref().map(|update| UpdateInfo {
        version: update.version.clone(),
        notes: update.body.clone().unwrap_or_default(),
        date: update.date.map(|date| date.to_string()).unwrap_or_default(),
    });
    *runtime.updates.offered.lock().unwrap() = update;
    Ok(info)
}
#[tauri::command]
async fn install_update(
    app: tauri::AppHandle,
    state: State<'_, Arc<Runtime>>,
) -> Result<(), String> {
    let runtime = state.inner().clone();
    runtime.updates.start_install()?;
    let update = runtime.updates.offered.lock().unwrap().clone();
    let result = if let Some(update) = update {
        let mut bytes = 0u64;
        let mut previous = std::time::Instant::now();
        update
            .download_and_install(
                move |chunk, total| {
                    bytes += chunk as u64;
                    if previous.elapsed() > std::time::Duration::from_millis(100) {
                        let _ = app.emit(
                            "update-progress",
                            serde_json::json!({"bytes":bytes,"total":total.unwrap_or(0)}),
                        );
                        previous = std::time::Instant::now();
                    }
                },
                || {},
            )
            .await
            .map_err(|error| error.to_string())
    } else {
        Err("update_not_checked".into())
    };
    runtime
        .updates
        .installing
        .store(false, std::sync::atomic::Ordering::Release);
    result
}
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            bootstrap,
            recording_action,
            list_videos,
            start_file_job,
            cancel_file_job,
            save_settings,
            reconnect,
            play_feedback,
            check_update,
            install_update
        ])
        .setup(|app| {
            let config = app.path().app_config_dir()?.join("settings.json");
            let mut errors = vec![];
            let saved = settings::load(&config).unwrap_or_else(|_| {
                errors.push("settings_load_failed".into());
                Settings::default()
            });
            let handle = app.handle().clone();
            let sink: EventSink = Arc::new(move |event, payload| {
                let _ = handle.emit(event, payload);
            });
            let resources = app.path().resource_dir()?;
            let runtime = Arc::new(Runtime {
                engine: RwLock::new(None),
                files: FileService::start(resources.clone(), sink.clone()),
                settings: Mutex::new(saved),
                settings_path: config,
                resources,
                sink,
                errors: Mutex::new(errors),
                reconnect: Mutex::new(()),
                updates: Updates::default(),
            });
            if let Err(error) = runtime.reconnect() {
                runtime.errors.lock().unwrap().push(error);
            }
            app.manage(runtime);
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("Failed to build TakeDock");
    app.run(|handle, event| {
        if let tauri::RunEvent::Exit = event {
            handle.state::<Arc<Runtime>>().shutdown();
        }
    });
}
