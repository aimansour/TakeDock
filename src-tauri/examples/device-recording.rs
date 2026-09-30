//! Explicit real-device acceptance helper. Creates a test recording.
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use takedock_lib::{
    model::{RecordingAction, RecordingState, Settings},
    session::Engine,
};
fn wait_for(engine: &Engine, state: RecordingState) -> Result<(), String> {
    let start = Instant::now();
    while engine.snapshot().observed != state || engine.snapshot().pending != 0 {
        if start.elapsed() > Duration::from_secs(15) {
            return Err(format!(
                "Camera did not reach {state:?}; condition={}",
                engine.snapshot().condition
            ));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    Ok(())
}
fn main() -> Result<(), String> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let apk = args
        .first()
        .ok_or("Usage: device-recording OBSERVER_APK [ADB_EXECUTABLE]")?;
    let settings = Settings {
        adb_path: args.get(1).cloned().unwrap_or_default(),
        ..Default::default()
    };
    let engine = Engine::start(
        settings,
        apk.into(),
        Arc::new(|event, value| {
            if event == "operation-error" {
                eprintln!("{}", value["code"]);
            }
        }),
    )
    .map_err(|_| "Session startup failed".to_string())?;
    wait_for(&engine, RecordingState::Idle)?;
    for (action, state) in [
        (RecordingAction::Start, RecordingState::Recording),
        (RecordingAction::Pause, RecordingState::Paused),
        (RecordingAction::Resume, RecordingState::Recording),
        (RecordingAction::Stop, RecordingState::Idle),
    ] {
        if action == RecordingAction::Stop {
            std::thread::sleep(Duration::from_millis(800));
        }
        let started = Instant::now();
        let receipt = engine.dispatch(action)?;
        let dispatch = started.elapsed();
        wait_for(&engine, state)?;
        println!(
            "{action:?}: sequence={}, dispatch_us={}, observed_ms={}",
            receipt.sequence,
            dispatch.as_micros(),
            started.elapsed().as_millis()
        );
    }
    engine.shutdown();
    Ok(())
}
