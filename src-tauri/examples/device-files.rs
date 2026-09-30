//! Explicit, destructive acceptance ONLY for paths in a caller-supplied test manifest.
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use takedock_lib::{
    files::{FileService, JobKind},
    model::Settings,
    session::Engine,
};
fn wait(mut condition: impl FnMut() -> bool) -> Result<(), String> {
    let start = Instant::now();
    while !condition() {
        if start.elapsed() > Duration::from_secs(60) {
            return Err("Acceptance timeout".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    Ok(())
}
fn main() -> Result<(), String> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 4 {
        return Err(
            "Usage: device-files RESOURCE_DIRECTORY ADB TEST_MANIFEST EMPTY_DESTINATION".into(),
        );
    }
    let resources = PathBuf::from(&args[0]);
    let destination = PathBuf::from(&args[3]);
    if std::fs::read_dir(&destination)
        .map_err(|e| e.to_string())?
        .next()
        .is_some()
    {
        return Err("Acceptance destination must be empty".into());
    }
    let allowed = std::fs::read_to_string(&args[2])
        .map_err(|e| e.to_string())?
        .lines()
        .map(|line| line.rsplit('/').next().unwrap_or("").trim().to_string())
        .collect::<Vec<_>>();
    if allowed.len() != 2 || allowed.iter().any(|name| !name.starts_with("VID_")) {
        return Err("Exactly two dedicated Open Camera test paths required".into());
    }
    let events = Arc::new(Mutex::new(vec![]));
    let output = events.clone();
    let sink: takedock_lib::session::EventSink = Arc::new(move |name, value| {
        if name == "file-job" {
            output.lock().unwrap().push(value)
        }
    });
    let engine = Engine::start(
        Settings {
            adb_path: args[1].clone(),
            ..Default::default()
        },
        resources.join("observer.apk"),
        sink.clone(),
    )?;
    wait(|| engine.snapshot().connected)?;
    let files = FileService::start(resources, sink);
    let selected = files
        .list(engine.lease().unwrap())?
        .into_iter()
        .filter(|video| allowed.contains(&video.name))
        .collect::<Vec<_>>();
    if selected.len() != 2 || selected.iter().any(|video| !video.ready) {
        return Err("Test recordings missing or unfinished".into());
    }
    let job = |kind, videos, dest: PathBuf, stem| -> Result<(), String> {
        let id = files.start_job(engine.lease().unwrap(), kind, videos, dest, stem)?;
        wait(|| {
            events.lock().unwrap().iter().any(|e| {
                e["id"] == id
                    && ["completed", "failed", "cancelled"]
                        .contains(&e["status"].as_str().unwrap_or(""))
            })
        })?;
        let event = events
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find(|e| e["id"] == id)
            .unwrap()
            .clone();
        if event["status"] != "completed" {
            return Err(format!("{kind:?} {}", event["message"]));
        }
        Ok(())
    };
    job(JobKind::Copy, selected.clone(), destination.clone(), None)?;
    if job(
        JobKind::Move,
        vec![selected[0].clone()],
        destination.clone(),
        None,
    )
    .is_ok()
    {
        return Err("Collision not rejected".into());
    }
    let new_stem = format!("TakeDock-test-رحلة-{}", uuid::Uuid::new_v4());
    job(
        JobKind::Rename,
        vec![selected[0].clone()],
        PathBuf::new(),
        Some(new_stem.clone()),
    )?;
    let renamed = files
        .list(engine.lease().unwrap())?
        .into_iter()
        .find(|v| v.name.starts_with(&new_stem))
        .ok_or("Renamed fixture missing")?;
    let moved = destination.join("move");
    std::fs::create_dir(&moved).map_err(|e| e.to_string())?;
    job(JobKind::Move, vec![renamed], moved, None)?;
    job(
        JobKind::Delete,
        vec![selected[1].clone()],
        PathBuf::new(),
        None,
    )?;
    engine.shutdown();
    files.shutdown();
    println!(
        "Real-phone completed-video list, batch copy, collision retention, Unicode rename, verified move, and selected delete passed."
    );
    Ok(())
}
