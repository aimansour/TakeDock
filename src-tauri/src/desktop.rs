use crate::updates::{UpdateInfo, Updates};
use crate::{
    events::Events,
    files::{FileService, JobKind, Video},
    model::*,
    session::{Engine, EventSink},
    settings,
    windows::{self, Windows},
};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicBool, Ordering},
    },
};
use tauri::{Emitter, Manager, State};
use tauri_plugin_updater::UpdaterExt;

struct Runtime {
    engine: RwLock<Option<Arc<Engine>>>,
    files: FileService,
    settings: Arc<Mutex<Settings>>,
    settings_path: PathBuf,
    resources: PathBuf,
    sink: EventSink,
    errors: Mutex<Vec<String>>,
    reconnect: Mutex<()>,
    updates: Updates,
    windows: Mutex<Windows>,
    events: Arc<Mutex<Events>>,
    startup_checked: AtomicBool,
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
    window_name: String,
    jobs: Vec<serde_json::Value>,
    check_at_startup: bool,
}
#[tauri::command]
fn bootstrap(window: tauri::WebviewWindow, state: State<'_, Arc<Runtime>>) -> Bootstrap {
    Bootstrap {
        settings: state.settings.lock().unwrap().clone(),
        session: state
            .engine()
            .map(|engine| engine.snapshot())
            .unwrap_or_default(),
        errors: state.errors.lock().unwrap().clone(),
        window_name: state.windows.lock().unwrap().name(window.label()),
        jobs: state.events.lock().unwrap().jobs.clone(),
        check_at_startup: !state.startup_checked.swap(true, Ordering::AcqRel),
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
    mut settings: Settings,
    app: tauri::AppHandle,
    state: State<'_, Arc<Runtime>>,
) -> Result<Settings, String> {
    let runtime = state.inner().clone();
    let videos = app.path().video_dir().map_err(|error| error.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        settings::ensure_destination(&mut settings, &videos)?;
        let mut current = runtime.settings.lock().unwrap();
        settings::save(&runtime.settings_path, &settings)?;
        *current = settings.clone();
        let _ = app.emit("settings-changed", &settings);
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
    feedback(success);
}
fn feedback(success: bool) {
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
async fn new_window(app: tauri::AppHandle, state: State<'_, Arc<Runtime>>) -> Result<(), String> {
    let label = format!("window-{}", uuid::Uuid::new_v4());
    let language = state.settings.lock().unwrap().language.clone();
    let name = state.windows.lock().unwrap().add(&label, &language);
    let mut config = app.config().app.windows[0].clone();
    config.label = label.clone();
    config.title = windows::title(&name);
    let result =
        tauri::WebviewWindowBuilder::from_config(&app, &config).and_then(|builder| builder.build());
    match result {
        Ok(window) => window.set_focus().map_err(|error| error.to_string()),
        Err(error) => {
            state.windows.lock().unwrap().remove(&label);
            Err(error.to_string())
        }
    }
}
#[tauri::command]
fn rename_window(
    name: String,
    window: tauri::WebviewWindow,
    state: State<'_, Arc<Runtime>>,
) -> Result<String, String> {
    let name = state
        .windows
        .lock()
        .unwrap()
        .rename(window.label(), &name)?;
    window
        .set_title(&windows::title(&name))
        .map_err(|error| error.to_string())?;
    Ok(name)
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
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            let label = app
                .try_state::<Arc<Runtime>>()
                .and_then(|state| state.windows.lock().unwrap().active())
                .unwrap_or_else(|| "main".into());
            if let Some(window) = app.get_webview_window(&label) {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
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
            install_update,
            new_window,
            rename_window
        ])
        .setup(|app| {
            let config = app.path().app_config_dir()?.join("settings.json");
            let mut errors = vec![];
            let mut persist_defaults = true;
            let mut saved = settings::load(&config).unwrap_or_else(|_| {
                persist_defaults = false;
                errors.push("settings_load_failed".into());
                Settings::default()
            });
            let destination = app
                .path()
                .video_dir()
                .map_err(|error| error.to_string())
                .and_then(|videos| settings::ensure_destination(&mut saved, &videos));
            if destination.is_err() {
                errors.push("destination_setup_failed".into());
            } else if persist_defaults && settings::save(&config, &saved).is_err() {
                errors.push("settings_save_failed".into());
            }
            let mut windows = Windows::default();
            let name = windows.add("main", &saved.language);
            if let Some(window) = app.get_webview_window("main") {
                window.set_title(&windows::title(&name))?;
            }
            let settings = Arc::new(Mutex::new(saved));
            let events = Arc::new(Mutex::new(Events::default()));
            let handle = app.handle().clone();
            let event_settings = settings.clone();
            let history = events.clone();
            let sink: EventSink = Arc::new(move |event, payload| {
                let sound = history.lock().unwrap().receive(event, &payload);
                if let Some(success) = sound
                    && (!success || event_settings.lock().unwrap().success_sound)
                {
                    feedback(success);
                }
                let _ = handle.emit(event, payload);
            });
            let resources = app.path().resource_dir()?;
            let runtime = Arc::new(Runtime {
                engine: RwLock::new(None),
                files: FileService::start(resources.clone(), sink.clone()),
                settings,
                settings_path: config,
                resources,
                sink,
                errors: Mutex::new(errors),
                reconnect: Mutex::new(()),
                updates: Updates::default(),
                windows: Mutex::new(windows),
                events,
                startup_checked: AtomicBool::new(false),
            });
            if let Err(error) = runtime.reconnect() {
                runtime.errors.lock().unwrap().push(error);
            }
            app.manage(runtime);
            Ok(())
        })
        .on_window_event(|window, event| {
            if let Some(state) = window.app_handle().try_state::<Arc<Runtime>>() {
                match event {
                    tauri::WindowEvent::Focused(true) => {
                        state.windows.lock().unwrap().focus(window.label())
                    }
                    tauri::WindowEvent::Destroyed => {
                        state.windows.lock().unwrap().remove(window.label())
                    }
                    _ => {}
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("Failed to build TakeDock");
    app.run(|handle, event| {
        if let tauri::RunEvent::Exit = event {
            handle.state::<Arc<Runtime>>().shutdown();
        }
    });
}
