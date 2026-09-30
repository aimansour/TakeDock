use crate::model::Settings;
use std::io::Write;
use std::path::Path;

fn validate(settings: &Settings) -> Result<(), String> {
    if !["en", "ar"].contains(&settings.language.as_str())
        || !(2000..=120_000).contains(&settings.verification_ms)
    {
        return Err("invalid_settings".into());
    }
    if !settings.destination.is_empty() && !Path::new(&settings.destination).is_absolute() {
        return Err("destination_must_be_absolute".into());
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
