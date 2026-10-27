//! Destructive source-only acceptance, limited to exact identities in a fresh owned manifest.
use serde::Deserialize;
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use takedock_lib::{
    files::{FileService, JobKind},
    model::Settings,
    session::Engine,
};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    source_root: String,
    fixtures: Vec<Owned>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Owned {
    name: String,
    identity: String,
}
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
    if args.len() != 3 {
        return Err("Usage: device-files RESOURCE_DIRECTORY ADB FRESH_OWNED_MANIFEST_JSON".into());
    }
    let manifest: Manifest =
        serde_json::from_slice(&std::fs::read(&args[2]).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    if manifest.source_root != "/sdcard/DCIM/OpenCamera"
        || manifest.fixtures.len() != 3
        || manifest.fixtures.iter().any(|f| {
            !f.name.starts_with("TDHandoff-") || takedock_files::validate_name(&f.name).is_err()
        })
    {
        return Err(
            "Exactly three dedicated source fixtures with bound identities required".into(),
        );
    }
    let events = Arc::new(Mutex::new(Vec::new()));
    let output = events.clone();
    let sink: takedock_lib::session::EventSink = Arc::new(move |name, value| {
        if name == "file-job" {
            output.lock().unwrap().push(value);
        }
    });
    let engine = Engine::start(
        Settings {
            adb_path: args[1].clone(),
            ..Default::default()
        },
        std::path::Path::new(&args[0]).join("observer.apk"),
        sink.clone(),
    )?;
    wait(|| engine.snapshot().connected)?;
    let files = FileService::start(args[0].clone().into(), sink);
    let operation = || -> Result<(), String> {
        let listed = files.list(engine.lease()?)?;
        let selected = manifest
            .fixtures
            .iter()
            .map(|owned| {
                listed
                    .iter()
                    .find(|v| v.name == owned.name && v.identity == owned.identity && v.ready)
                    .cloned()
                    .ok_or_else(|| "Owned fixture missing, replaced or unfinished".to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let job = |kind, videos, stem| -> Result<(), String> {
            let id = files.start_job(engine.lease()?, kind, videos, stem, None)?;
            wait(|| {
                events.lock().unwrap().iter().any(|e| {
                    e["id"] == id
                        && ["completed", "failed", "cancelled"]
                            .contains(&e["status"].as_str().unwrap_or(""))
                })
            })?;
            let events = events.lock().unwrap();
            let event = events.iter().rev().find(|e| e["id"] == id).unwrap();
            if event["status"] != "completed" {
                return Err(format!("{kind:?}: {}", event["message"]));
            }
            Ok(())
        };
        let stem = format!("TDHandoff-renamed-رحلة-{}", uuid::Uuid::new_v4());
        job(
            JobKind::Rename,
            vec![selected[0].clone()],
            Some(stem.clone()),
        )?;
        let renamed = files
            .list(engine.lease()?)?
            .into_iter()
            .find(|v| v.name == format!("{stem}.mp4"))
            .ok_or("Renamed fixture missing")?;
        job(JobKind::Mark, vec![renamed, selected[2].clone()], None)?;
        job(JobKind::Delete, vec![selected[1].clone()], None)?;
        Ok(())
    };
    let result = operation();
    files.shutdown();
    engine.shutdown();
    result?;
    println!(
        "Owned source rename, batch handoff and delete passed; no destination collection was accessed."
    );
    Ok(())
}
