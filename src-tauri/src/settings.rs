use crate::model::Settings;
use std::io::Write;
use std::path::Path;

fn validate(settings: &Settings) -> Result<(), String> {
    if !["en", "ar"].contains(&settings.language.as_str())
        || !(2000..=120_000).contains(&settings.verification_ms)
        || !(500..=5000).contains(&settings.ready_delay_ms)
        || !settings.ready_delay_ms.is_multiple_of(50)
    {
        return Err("invalid_settings".into());
    }
    if !settings.adb_path.is_empty() && !Path::new(&settings.adb_path).is_absolute() {
        return Err("adb_path_must_be_absolute".into());
    }
    Ok(())
}
pub fn load(path: &Path) -> Result<Settings, String> {
    if !path.exists() {
        return Ok(Settings::default());
    }
    let settings = serde_json::from_slice::<Settings>(
        &std::fs::read(path).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    validate(&settings)?;
    Ok(settings)
}
pub fn save(path: &Path, settings: &Settings) -> Result<(), String> {
    validate(settings)?;
    let parent = path.parent().ok_or("settings_parent_missing")?;
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let mut temporary =
        tempfile::NamedTempFile::new_in(parent).map_err(|error| error.to_string())?;
    temporary
        .write_all(&serde_json::to_vec_pretty(settings).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| error.to_string())?;
    temporary.persist(path).map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_destination_is_ignored_without_filesystem_access() {
        let trial = tempfile::tempdir().unwrap();
        let path = trial.path().join("settings.json");
        let unused = trial.path().join("must-not-create");
        let adb = trial.path().join("adb.exe");
        for obsolete in [
            serde_json::json!(unused),
            serde_json::json!("relative/obsolete"),
            serde_json::json!({"obsolete":true}),
        ] {
            std::fs::write(&path, serde_json::to_vec(&serde_json::json!({
                "language":"ar","destination":obsolete,"adb_path":adb,
                "success_sound":true,"ready_sound":false,"ready_delay_ms":1250,"verification_ms":15000
            })).unwrap()).unwrap();
            let loaded = load(&path).unwrap();
            assert_eq!(loaded.language, "ar");
            assert_eq!(loaded.adb_path, adb.to_str().unwrap());
            assert!(loaded.success_sound);
            assert!(!loaded.ready_sound);
            assert_eq!(
                (loaded.ready_delay_ms, loaded.verification_ms),
                (1250, 15000)
            );
            save(&path, &loaded).unwrap();
            let saved: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            assert!(saved.get("destination").is_none());
            assert!(!unused.exists());
        }
    }
    #[test]
    fn legacy_settings_default_readiness_without_replacing_existing_values() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        let destination = directory
            .path()
            .join("My videos")
            .to_str()
            .unwrap()
            .to_owned();
        let adb_path = directory
            .path()
            .join("adb.exe")
            .to_str()
            .unwrap()
            .to_owned();
        let legacy = serde_json::json!({"language":"ar", "destination":destination,
            "adb_path":adb_path, "success_sound":true, "verification_ms":15000});
        std::fs::write(&path, legacy.to_string()).unwrap();
        let saved = load(&path).unwrap();
        assert_eq!(saved.language, "ar");
        assert!(
            serde_json::to_value(&saved)
                .unwrap()
                .get("destination")
                .is_none()
        );
        assert_eq!(saved.adb_path, adb_path);
        assert!(saved.success_sound);
        assert_eq!(saved.verification_ms, 15_000);
        assert_eq!((saved.ready_sound, saved.ready_delay_ms), (true, 800));
    }
    #[test]
    fn readiness_settings_survive_restart_independently_of_success_sound() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        let defaults = load(&path).unwrap();
        assert_eq!((defaults.ready_sound, defaults.ready_delay_ms), (true, 800));
        let settings = Settings {
            ready_sound: false,
            ready_delay_ms: 1250,
            success_sound: true,
            ..Default::default()
        };
        save(&path, &settings).unwrap();
        let saved = load(&path).unwrap();
        assert_eq!((saved.ready_sound, saved.ready_delay_ms), (false, 1250));
        assert!(saved.success_sound);
    }
    #[test]
    fn invalid_readiness_delay_preserves_the_last_good_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        for delay in [500, 800, 5000] {
            save(
                &path,
                &Settings {
                    ready_delay_ms: delay,
                    ..Default::default()
                },
            )
            .unwrap();
            assert_eq!(load(&path).unwrap().ready_delay_ms, delay);
        }
        let good = std::fs::read(&path).unwrap();
        for delay in [499, 501, 525, 5001] {
            assert_eq!(
                save(
                    &path,
                    &Settings {
                        ready_delay_ms: delay,
                        ..Default::default()
                    }
                ),
                Err("invalid_settings".into())
            );
            assert_eq!(std::fs::read(&path).unwrap(), good);
        }
    }
    #[test]
    fn new_session_serializes_inactive_speech_readiness() {
        let wire = serde_json::to_value(crate::model::SessionState::default()).unwrap();
        assert_eq!(wire["speech_readiness"], "inactive");
    }
    #[test]
    fn defaults_are_silent_and_survive_restart_after_edit() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        assert!(!load(&path).unwrap().success_sound);
        let settings = Settings {
            language: "ar".into(),
            success_sound: true,
            ..Default::default()
        };
        save(&path, &settings).unwrap();
        let persisted = load(&path).unwrap();
        assert!(persisted.success_sound);
        assert_eq!(persisted.language, "ar");
    }
    #[test]
    fn invalid_settings_do_not_replace_last_good_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        save(&path, &Settings::default()).unwrap();
        assert!(
            save(
                &path,
                &Settings {
                    verification_ms: 0,
                    ..Default::default()
                }
            )
            .is_err()
        );
        assert_eq!(load(&path).unwrap().verification_ms, 10_000);
    }
    #[test]
    fn corrupt_settings_are_reported_not_silently_discarded() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        std::fs::write(&path, b"broken").unwrap();
        assert!(load(&path).is_err());
    }
}
