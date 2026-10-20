use crate::{
    files::{Video, hash_path, quote},
    session::DeviceLease,
};
use std::path::PathBuf;
use takedock_files::Identity;

pub(crate) struct HelperClient {
    resources: PathBuf,
    deployed: u64,
    path: String,
}
impl HelperClient {
    pub(crate) fn new(resources: PathBuf) -> Self {
        Self {
            resources,
            deployed: 0,
            path: String::new(),
        }
    }
    pub(crate) fn prepare(&mut self, lease: &DeviceLease) -> Result<(), String> {
        if self.deployed == lease.generation {
            return Ok(());
        }
        let abi =
            String::from_utf8(lease.output(&["shell", "getprop", "ro.product.cpu.abi"], 5000)?)
                .map_err(|e| e.to_string())?;
        let abi = abi.trim();
        if !["arm64-v8a", "armeabi-v7a", "x86_64"].contains(&abi) {
            return Err("unsupported_device_abi".into());
        }
        let local = self
            .resources
            .join("file-helper")
            .join(abi)
            .join("takedock-files");
        let digest = hash_path(&local)?;
        let path = format!("/data/local/tmp/takedock-files-{digest}");
        let existing = lease
            .output(&["shell", "sha256sum", &path], 5000)
            .ok()
            .and_then(|data| String::from_utf8(data).ok());
        if !existing
            .as_deref()
            .is_some_and(|text| text.split_whitespace().next() == Some(&digest))
        {
            let temporary = format!("{path}-{}", uuid::Uuid::new_v4());
            lease.output(
                &[
                    "push",
                    local.to_str().ok_or("resource_path_encoding")?,
                    &temporary,
                ],
                30_000,
            )?;
            lease.output(&["shell", "chmod", "700", &temporary], 5000)?;
            lease.output(&["shell", "mv", "-f", &temporary, &path], 5000)?;
        }
        self.path = path;
        self.deployed = lease.generation;
        Ok(())
    }
    pub(crate) fn command(&self, args: &[&str]) -> Result<String, String> {
        std::iter::once(self.path.as_str())
            .chain(args.iter().copied())
            .map(quote)
            .collect::<Result<Vec<_>, _>>()
            .map(|args| args.join(" "))
    }
    pub(crate) fn output(
        &self,
        lease: &DeviceLease,
        args: &[&str],
        timeout: u64,
    ) -> Result<Vec<u8>, String> {
        lease.output(&["exec-out", &self.command(args)?], timeout)
    }
    pub(crate) fn stat(&self, lease: &DeviceLease, video: &Video) -> Result<(), String> {
        let identity: Identity =
            serde_json::from_slice(&self.output(lease, &["stat", &video.name], 10_000)?)
                .map_err(|e| e.to_string())?;
        let expected: Identity =
            serde_json::from_str(&video.identity).map_err(|e| e.to_string())?;
        if identity != expected {
            return Err("source_changed".into());
        }
        Ok(())
    }
}
