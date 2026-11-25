use anyhow::{Context, Result};
use serde::Deserialize;
use sha1::{Digest, Sha1};
use std::{path::{Path, PathBuf}, io::Write};

const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

#[derive(Debug, Deserialize)]
pub struct Manifest {
    pub latest: Latest,
    pub versions: Vec<VersionEntry>,
}
#[derive(Debug, Deserialize)]
pub struct Latest { pub release: String, pub snapshot: String }
#[derive(Debug, Deserialize, Clone)]
pub struct VersionEntry {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct VersionInfo {
    pub id: String,
    pub downloads: Downloads,
}
#[derive(Debug, Deserialize)]
pub struct Downloads { pub client: DownloadInfo }
#[derive(Debug, Deserialize)]
pub struct DownloadInfo { pub url: String, pub sha1: String, pub size: u64 }

pub fn game_dir() -> Result<PathBuf> {
    if let Some(custom) = std::env::var_os("FPK_HOME") {
        return Ok(PathBuf::from(custom));
    }
    let base = dirs_path()?;
    Ok(base.join("fpk-launcher"))
}
fn dirs_path() -> Result<PathBuf> {
    #[cfg(target_os = "windows")]
    { return Ok(PathBuf::from(std::env::var_os("APPDATA").context("APPDATA not set")?)); }
    #[cfg(target_os = "macos")]
    { return Ok(PathBuf::from(std::env::var_os("HOME").context("HOME not set")?).join("Library/Application Support")); }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    { Ok(PathBuf::from(std::env::var_os("XDG_DATA_HOME").unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/share").into_os_string()))) }
}

pub async fn fetch_manifest() -> Result<Manifest> {
    reqwest::Client::new().get(MANIFEST_URL).send().await?
        .error_for_status()?.json().await.context("invalid Mojang version manifest")
}

pub async fn install_client(version: &str) -> Result<PathBuf> {
    let manifest = fetch_manifest().await?;
    let entry = manifest.versions.iter().find(|v| v.id == version)
        .context("version not found in Mojang manifest")?;
    let root = game_dir()?;
    let version_dir = root.join("versions").join(version);
    tokio::fs::create_dir_all(&version_dir).await?;
    let info: VersionInfo = reqwest::get(&entry.url).await?.error_for_status()?.json().await?;
    let json_path = version_dir.join(format!("{version}.json"));
    tokio::fs::write(&json_path, serde_json::to_vec_pretty(&info)?).await?;
    let jar_path = version_dir.join(format!("{version}.jar"));
    let bytes = reqwest::get(&info.downloads.client.url).await?.error_for_status()?.bytes().await?;
    if bytes.len() as u64 != info.downloads.client.size {
        anyhow::bail!("download size mismatch; refusing to keep incomplete JAR");
    }
    let actual_sha1 = hex::encode(Sha1::digest(&bytes));
    if actual_sha1 != info.downloads.client.sha1.to_ascii_lowercase() {
        anyhow::bail!("SHA-1 mismatch for downloaded client; expected {}, got {}", info.downloads.client.sha1, actual_sha1);
    }
    let temp_path = version_dir.join(format!("{version}.jar.part"));
    let mut file = std::fs::File::create(&temp_path)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    std::fs::rename(&temp_path, &jar_path)?;
    println!("Client downloaded and verified: {}", jar_path.display());
    Ok(jar_path)
}

pub fn list_versions(manifest: &Manifest, kind: &str) -> Vec<&VersionEntry> {
    manifest.versions.iter().filter(|v| kind == "all" || v.kind == kind).collect()
}

pub fn ensure_path(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path)?;
    Ok(())
}
