use crate::Paths;
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use tempfile::NamedTempFile;

pub fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}
pub fn hash(data: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(data.as_ref()))
}
pub fn read_json(path: &Path) -> Result<Value> {
    serde_json::from_slice(&fs::read(path).with_context(|| format!("Read {}", path.display()))?)
        .with_context(|| format!("Invalid JSON: {}", path.display()))
}
pub fn write_json(path: &Path, value: &Value) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    atomic(path, &bytes)
}
pub fn stage(path: &Path, bytes: &[u8], directory: &Path) -> Result<NamedTempFile> {
    fs::create_dir_all(directory)?;
    let mut temp = NamedTempFile::new_in(directory)?;
    temp.write_all(bytes)?;
    if let Ok(meta) = fs::metadata(path) {
        temp.as_file().set_permissions(meta.permissions())?;
    }
    temp.as_file().sync_all()?;
    Ok(temp)
}
pub fn atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().context("Path has no parent")?;
    stage(path, bytes, parent)?
        .persist(path)
        .map_err(|e| e.error)
        .with_context(|| format!("Replace {}", path.display()))?;
    Ok(())
}
pub fn mode(path: &Path, bits: u32) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(bits))?;
    }
    #[cfg(not(unix))]
    {
        let _ = (path, bits);
    }
    Ok(())
}
pub fn backup(paths: &Paths, path: &Path) -> Result<PathBuf> {
    let dest = paths
        .state
        .join("backups")
        .join(&hash(path.to_string_lossy().as_bytes())[..16]);
    fs::create_dir_all(dest.parent().unwrap())?;
    mode(dest.parent().unwrap(), 0o700)?;
    let meta = dest.with_extension("json");
    if !meta.exists() {
        if path.exists() {
            atomic(&dest, &fs::read(path)?)?;
            mode(&dest, 0o600)?;
        }
        write_json(
            &meta,
            &json!({"path":path,"existed":path.exists(),"saved_at":now()}),
        )?;
        mode(&meta, 0o600)?;
    }
    Ok(dest)
}
pub fn strip_block(text: &str) -> String {
    regex::Regex::new(r"(?s)\n?# paintings:start\n.*?# paintings:end\n?")
        .unwrap()
        .replace_all(text, "\n")
        .into_owned()
}
pub fn managed_block(paths: &Paths, path: &Path, content: &str) -> Result<()> {
    backup(paths, path)?;
    let old = if path.exists() {
        fs::read_to_string(path)?
    } else {
        String::new()
    };
    atomic(
        path,
        format!(
            "{}\n\n# paintings:start\n{}\n# paintings:end\n",
            strip_block(&old).trim_end(),
            content.trim_end()
        )
        .as_bytes(),
    )
}
pub fn lock(path: &Path, wait: bool) -> Result<Option<File>> {
    fs::create_dir_all(path.parent().context("Lock has no parent")?)?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)?;
    let result = if wait {
        fs2::FileExt::lock_exclusive(&file)
    } else {
        fs2::FileExt::try_lock_exclusive(&file)
    };
    match result {
        Ok(()) => Ok(Some(file)),
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
        Err(e) => Err(e.into()),
    }
}
pub fn fetch(url: &str, limit: u64, seconds: u64) -> Result<Vec<u8>> {
    if url::Url::parse(url)?.scheme() != "https" {
        bail!("Downloads require HTTPS");
    }
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .https_only(true)
        .timeout_global(Some(std::time::Duration::from_secs(seconds)))
        .build()
        .into();
    Ok(agent
        .get(url)
        .header(
            "User-Agent",
            "Paintings/0.3 (https://github.com/bestquark/paintings)",
        )
        .call()?
        .body_mut()
        .with_config()
        .limit(limit)
        .read_to_vec()?)
}
