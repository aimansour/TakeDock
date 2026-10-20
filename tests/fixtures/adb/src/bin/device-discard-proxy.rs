//! Private physical-trial preservation boundary. Never packaged with TakeDock.
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use takedock_files::Identity;

struct Target {
    serial: String,
    helper: String,
    name: String,
    identity: Identity,
    operation: String,
}
fn approved_target(args: &[String], manifest: Option<&Value>) -> Result<Option<Target>, String> {
    let raw = args.last().map(String::as_str).unwrap_or("");
    let words = shell_words::split(raw).map_err(|_| "invalid_command")?;
    if words
        .get(1)
        .is_some_and(|s| ["delete", "rename"].contains(&s.as_str()))
        && words[0].starts_with("/data/local/tmp/takedock-files-")
    {
        return Err("unguarded_trial_mutation".into());
    }
    if !words.iter().any(|s| s == "delete-guarded") {
        if raw.contains("delete-guarded") {
            return Err("invalid_guarded_command".into());
        }
        return Ok(None);
    }
    if args.len() != 5
        || args[0] != "-s"
        || args[2] != "shell"
        || args[3] != "-T"
        || words.len() != 5
        || words[1] != "delete-guarded"
    {
        return Err("invalid_guarded_command".into());
    }
    let m = manifest.ok_or("missing_allowlist")?;
    let helper = &words[0];
    let digest = helper
        .strip_prefix("/data/local/tmp/takedock-files-")
        .ok_or("invalid_helper")?;
    if digest.len() != 64
        || !digest.bytes().all(|b| b.is_ascii_hexdigit())
        || m["version"] != 1
        || m["serial"].as_str() != Some(&args[1])
        || m["helper"].as_str() != Some(helper)
    {
        return Err("allowlist_mismatch".into());
    }
    let pre = m["pre_start_ms"].as_u64().ok_or("missing_pre_start_time")?;
    let start = m["take_started_ms"].as_u64().ok_or("missing_start_time")?;
    if pre >= start {
        return Err("invalid_pre_start_journal".into());
    }
    takedock_files::validate_name(&words[2])?;
    let identity: Identity = serde_json::from_str(&words[3]).map_err(|_| "invalid_identity")?;
    let prior = m["pre_start"]
        .as_array()
        .ok_or("missing_pre_start_manifest")?;
    for entry in prior {
        let old: Identity = serde_json::from_str(
            entry["identity"]
                .as_str()
                .ok_or("invalid_pre_start_identity")?,
        )
        .map_err(|_| "invalid_pre_start_identity")?;
        if entry["name"].as_str() == Some(&words[2])
            || (old.device == identity.device && old.inode == identity.inode)
        {
            return Err("target_was_in_pre_start_manifest".into());
        }
    }
    let entries = m["takes"].as_array().ok_or("missing_take_allowlist")?;
    let matches = entries
        .iter()
        .filter(|entry| {
            entry["name"].as_str() == Some(&words[2])
                && entry["identity"]
                    .as_str()
                    .and_then(|s| serde_json::from_str::<Identity>(s).ok())
                    .as_ref()
                    == Some(&identity)
        })
        .count();
    if matches != 1 {
        return Err("target_not_allowlisted".into());
    }
    Ok(Some(Target {
        serial: args[1].clone(),
        helper: helper.clone(),
        name: words[2].clone(),
        identity,
        operation: words[4].clone(),
    }))
}
fn preserve_then_forward<T>(
    target: &Target,
    preserve: impl FnOnce(&Target) -> Result<(), String>,
    forward: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    preserve(target)?;
    forward()
}
fn local_hash(path: &Path) -> Result<String, String> {
    let mut input = File::open(path).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut buf = [0; 65536];
    loop {
        let n = input.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
    }
    Ok(hash
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}
fn unique_file(directory: &Path, extension: &str) -> Result<(PathBuf, File), String> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let path = directory.join(format!("{}-{stamp}.{extension}", std::process::id()));
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    Ok((path, file))
}
fn run_bounded(command: &mut Command, deadline: Instant) -> Result<i32, String> {
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(status) => return Ok(status.code().unwrap_or(1)),
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("preservation_deadline".into());
            }
            None => std::thread::sleep(Duration::from_millis(10)),
        }
    }
}
struct Backup {
    adb: PathBuf,
    ffmpeg: PathBuf,
    directory: PathBuf,
    deadline: Instant,
}
impl Backup {
    fn helper(&self, target: &Target, kind: &str, output: File) -> Result<(), String> {
        let identity = serde_json::to_string(&target.identity).map_err(|e| e.to_string())?;
        let mut words = vec![target.helper.as_str(), kind, target.name.as_str()];
        if kind != "stat" {
            words.push(identity.as_str());
        }
        let remote = shell_words::join(words);
        let result = run_bounded(
            Command::new(&self.adb)
                .args(["-s", &target.serial, "exec-out", &remote])
                .stdin(Stdio::null())
                .stdout(output),
            self.deadline,
        )?;
        if result != 0 {
            return Err("backup_adb_failed".into());
        }
        Ok(())
    }
    fn text(&self, target: &Target, kind: &str) -> Result<String, String> {
        let (path, file) = unique_file(&self.directory, "metadata")?;
        let result = (|| {
            self.helper(target, kind, file)?;
            if fs::metadata(&path).map_err(|e| e.to_string())?.len() > 65536 {
                return Err("backup_metadata_too_large".into());
            }
            fs::read_to_string(&path).map_err(|e| e.to_string())
        })();
        let _ = fs::remove_file(path);
        result
    }
    fn preserve(&self, target: &Target) -> Result<(), String> {
        let (path, file) = unique_file(&self.directory, "partial")?;
        // Never erase even a partial backup on failure; diagnostics remain private.
        self.helper(target, "read", file)?;
        let local = local_hash(&path)?;
        if self.text(target, "hash")?.trim() != local {
            return Err("backup_hash_mismatch".into());
        }
        let current: Identity = serde_json::from_str(self.text(target, "stat")?.trim())
            .map_err(|_| "backup_stat_invalid")?;
        if current != target.identity {
            return Err("backup_source_changed".into());
        }
        let result = run_bounded(
            Command::new(&self.ffmpeg)
                .args(["-nostdin", "-v", "error", "-xerror", "-i"])
                .arg(&path)
                .args(["-f", "null", "-"])
                .stdin(Stdio::null())
                .stdout(Stdio::null()),
            self.deadline,
        )?;
        if result != 0 {
            return Err("backup_decode_failed".into());
        }
        // Stat again after decode so the published receipt covers the entire backup.
        let current: Identity = serde_json::from_str(self.text(target, "stat")?.trim())
            .map_err(|_| "backup_stat_invalid")?;
        if current != target.identity {
            return Err("backup_source_changed".into());
        }
        let complete = path.with_extension("mp4");
        fs::rename(&path, &complete).map_err(|e| e.to_string())?;
        OpenOptions::new()
            .write(true)
            .open(&complete)
            .and_then(|f| f.sync_all())
            .map_err(|e| format!("backup_sync: {e}"))?;
        let receipt = serde_json::json!({"serial":target.serial,"name":target.name,"identity":serde_json::to_string(&target.identity).unwrap(),"operation":target.operation,"sha256":local,"backup":complete,"fully_decoded":true});
        let (receipt_path, mut receipt_file) = unique_file(&self.directory, "receipt.json")?;
        std::io::Write::write_all(&mut receipt_file, receipt.to_string().as_bytes())
            .map_err(|e| e.to_string())?;
        receipt_file.sync_all().map_err(|e| e.to_string())?;
        eprintln!(
            "preservation verified; private receipt: {}",
            receipt_path.display()
        );
        Ok(())
    }
}
fn required_path(name: &str) -> Result<PathBuf, String> {
    std::env::var_os(name)
        .map(PathBuf::from)
        .ok_or_else(|| format!("missing {name}"))
}
fn run() -> Result<i32, String> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let adb = required_path("TAKEDOCK_TRIAL_REAL_ADB")?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if adb
        == std::env::current_exe()
            .map_err(|e| e.to_string())?
            .canonicalize()
            .map_err(|e| e.to_string())?
    {
        return Err("proxy_recursion".into());
    }
    let manifest = std::env::var_os("TAKEDOCK_TRIAL_ALLOWLIST")
        .and_then(|p| fs::read(p).ok())
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok());
    if let Some(target) = approved_target(&args, manifest.as_ref())? {
        let directory = required_path("TAKEDOCK_TRIAL_BACKUP_DIR")?;
        fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        let deadline = Instant::now() + Duration::from_secs(27);
        let backup = Backup {
            adb: adb.clone(),
            ffmpeg: required_path("TAKEDOCK_TRIAL_FFMPEG")?,
            directory,
            deadline: Instant::now() + Duration::from_secs(20),
        };
        preserve_then_forward(
            &target,
            |t| backup.preserve(t),
            || run_bounded(Command::new(&adb).args(&args), deadline),
        )
    } else {
        Command::new(&adb)
            .args(&args)
            .status()
            .map(|s| s.code().unwrap_or(1))
            .map_err(|e| e.to_string())
    }
}

// Kill inherited real-ADB/ffmpeg children if the host cancels this wrapper.
// ABI: Microsoft's JOBOBJECT_EXTENDED_LIMIT_INFORMATION and basic limit structs.
#[cfg(windows)]
fn isolate_children() -> Result<(), String> {
    use std::ffi::c_void;
    #[repr(C)]
    struct Basic {
        process_time: i64,
        job_time: i64,
        flags: u32,
        min: usize,
        max: usize,
        active: u32,
        affinity: usize,
        priority: u32,
        scheduling: u32,
    }
    #[repr(C)]
    struct Extended {
        basic: Basic,
        io: [u64; 6],
        process_memory: usize,
        job_memory: usize,
        peak_process: usize,
        peak_job: usize,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateJobObjectW(attributes: *const c_void, name: *const u16) -> *mut c_void;
        fn SetInformationJobObject(
            job: *mut c_void,
            class: i32,
            info: *const c_void,
            size: u32,
        ) -> i32;
        fn AssignProcessToJobObject(job: *mut c_void, process: *mut c_void) -> i32;
        fn GetCurrentProcess() -> *mut c_void;
    }
    // The unnamed handle is non-inheritable and intentionally process-lifetime.
    // On process exit Windows closes it, terminating all remaining descendants.
    unsafe {
        let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if job.is_null() {
            return Err("trial_job_create_failed".into());
        }
        let mut info: Extended = std::mem::zeroed();
        info.basic.flags = 0x2000;
        if SetInformationJobObject(
            job,
            9,
            &info as *const _ as *const c_void,
            std::mem::size_of::<Extended>() as u32,
        ) == 0
            || AssignProcessToJobObject(job, GetCurrentProcess()) == 0
        {
            return Err("trial_job_isolation_failed".into());
        }
    }
    Ok(())
}
#[cfg(not(windows))]
fn isolate_children() -> Result<(), String> {
    Err("physical_trial_proxy_requires_windows".into())
}

fn main() {
    let result = isolate_children().and_then(|()| run());
    match result {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("trial preservation rejected: {error}");
            std::process::exit(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::{Arc, Barrier};
    fn identity() -> String {
        r#"{"device":1,"inode":2,"size":3,"modified_ns":4,"changed_ns":5}"#.into()
    }
    fn args() -> Vec<String> {
        vec![
            "-s".into(),
            "owned-phone".into(),
            "shell".into(),
            "-T".into(),
            format!(
                "/data/local/tmp/takedock-files-{} delete-guarded take.mp4 '{}' op-1",
                "a".repeat(64),
                identity()
            ),
        ]
    }
    fn manifest() -> serde_json::Value {
        json!({"version":1,"serial":"owned-phone","helper":format!("/data/local/tmp/takedock-files-{}", "a".repeat(64)),"pre_start_ms":10,"take_started_ms":20,"pre_start":[],"takes":[{"name":"take.mp4","identity":identity()}]})
    }
    #[test]
    fn missing_or_wrong_allowlist_never_forwards_delete() {
        assert!(approved_target(&args(), None).is_err());
        for key in ["serial", "helper"] {
            let mut m = manifest();
            m[key] = json!("wrong");
            assert!(approved_target(&args(), Some(&m)).is_err());
        }
        let mut m = manifest();
        m["takes"][0]["identity"] = json!(identity().replace("\"inode\":2", "\"inode\":99"));
        assert!(approved_target(&args(), Some(&m)).is_err());
        let mut m = manifest();
        m["pre_start"] = json!([{"name":"take.mp4","identity":identity()}]);
        assert!(approved_target(&args(), Some(&m)).is_err());
        let mut m = manifest();
        m["pre_start_ms"] = json!(21);
        assert!(approved_target(&args(), Some(&m)).is_err());
    }
    #[test]
    fn failed_backup_verification_never_forwards_delete() {
        let target = approved_target(&args(), Some(&manifest()))
            .unwrap()
            .unwrap();
        let mut forwarded = false;
        assert!(
            preserve_then_forward(
                &target,
                |_| Err("decode_failed".into()),
                || {
                    forwarded = true;
                    Ok(())
                }
            )
            .is_err()
        );
        assert!(!forwarded);
    }
    #[test]
    fn watch_and_capture_continue_while_backup_is_pending() {
        let barrier = Arc::new(Barrier::new(2));
        let ready = Arc::new(Barrier::new(2));
        let b = barrier.clone();
        let r = ready.clone();
        let worker = std::thread::spawn(move || {
            let target = approved_target(&args(), Some(&manifest()))
                .unwrap()
                .unwrap();
            preserve_then_forward(
                &target,
                |_| {
                    r.wait();
                    b.wait();
                    Ok(())
                },
                || Ok(()),
            )
            .unwrap();
        });
        ready.wait();
        for command in ["", "/data/local/tmp/takedock-files-abc watch stream"] {
            let args = vec![
                "-s".into(),
                "owned-phone".into(),
                "shell".into(),
                command.into(),
            ];
            assert!(approved_target(&args, None).unwrap().is_none());
        }
        barrier.wait();
        worker.join().unwrap();
    }
    #[test]
    fn verified_backup_forwards_once_and_hashes_exact_bytes() {
        let target = approved_target(&args(), Some(&manifest()))
            .unwrap()
            .unwrap();
        let mut count = 0;
        preserve_then_forward(
            &target,
            |_| Ok(()),
            || {
                count += 1;
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(count, 1);
        let temp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(temp.path(), b"abc").unwrap();
        assert_eq!(
            local_hash(temp.path()).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
