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

#[derive(Debug, Deserialize, serde::Serialize)]
pub struct VersionInfo {
    pub id: String,
    pub downloads: Downloads,
}
#[derive(Debug, Deserialize, serde::Serialize)]
pub struct Downloads { pub client: DownloadInfo }
#[derive(Debug, Deserialize, serde::Serialize)]
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
    let metadata: serde_json::Value = reqwest::get(&entry.url).await?.error_for_status()?.json().await?;
    let client_info: DownloadInfo = serde_json::from_value(
        metadata.get("downloads").and_then(|v| v.get("client")).cloned()
            .context("version metadata has no client download")?
    )?;
    let json_path = version_dir.join(format!("{version}.json"));
    tokio::fs::write(&json_path, serde_json::to_vec_pretty(&metadata)?).await?;
    let jar_path = version_dir.join(format!("{version}.jar"));
    let bytes = reqwest::get(&client_info.url).await?.error_for_status()?.bytes().await?;
    if bytes.len() as u64 != client_info.size {
        anyhow::bail!("download size mismatch; refusing to keep incomplete JAR");
    }
    let actual_sha1 = hex::encode(Sha1::digest(&bytes));
    if actual_sha1 != client_info.sha1.to_ascii_lowercase() {
        anyhow::bail!("SHA-1 mismatch for downloaded client; expected {}, got {}", client_info.sha1, actual_sha1);
    }
    let temp_path = version_dir.join(format!("{version}.jar.part"));
    let mut file = std::fs::File::create(&temp_path)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    std::fs::rename(&temp_path, &jar_path)?;
    println!("Client downloaded and verified: {}", jar_path.display());
    Ok(jar_path)
}

pub fn list_versions<'a>(manifest: &'a Manifest, kind: &str) -> Vec<&'a VersionEntry> {
    manifest.versions.iter().filter(|v| kind == "all" || v.kind == kind).collect()
}

pub fn ensure_path(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path)?;
    Ok(())
}

#[derive(Debug, Deserialize)]
struct AssetIndexInfo { id: String, url: String, sha1: String, size: u64 }
#[derive(Debug, Deserialize)]
struct AssetObject { hash: String, size: u64 }
#[derive(Debug, Deserialize)]
struct LibraryInfo {
    downloads: Option<LibraryDownloads>,
    natives: Option<std::collections::HashMap<String, String>>,
    extract: Option<NativeExtract>,
}
#[derive(Debug, Deserialize)]
struct NativeExtract { exclude: Option<Vec<String>> }
#[derive(Debug, Deserialize)]
struct LibraryDownloads {
    artifact: Option<LibraryArtifact>,
    classifiers: Option<std::collections::HashMap<String, LibraryArtifact>>,
}
#[derive(Debug, Deserialize)]
struct LibraryArtifact { path: String, url: String, sha1: Option<String>, size: Option<u64> }

async fn download_checked(url: &str, target: &Path, sha1: Option<&str>, size: Option<u64>) -> Result<()> {
    if target.exists() {
        let cached = tokio::fs::read(target).await?;
        let size_ok = size.map_or(true, |expected| cached.len() as u64 == expected);
        let hash_ok = sha1.map_or(true, |expected| hex::encode(Sha1::digest(&cached)) == expected.to_ascii_lowercase());
        if size_ok && hash_ok { return Ok(()); }
        tokio::fs::remove_file(target).await.context("remove invalid cached download")?;
    }
    let bytes = reqwest::get(url).await?.error_for_status()?.bytes().await?;
    if let Some(expected) = size {
        if bytes.len() as u64 != expected { anyhow::bail!("download size mismatch for {}", target.display()); }
    }
    if let Some(expected) = sha1 {
        let actual = hex::encode(Sha1::digest(&bytes));
        if actual != expected.to_ascii_lowercase() {
            anyhow::bail!("SHA-1 mismatch for {}", target.display());
        }
    }
    if let Some(parent) = target.parent() { tokio::fs::create_dir_all(parent).await?; }
    let temporary = target.with_extension("fpk-part");
    let mut file = std::fs::File::create(&temporary)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    std::fs::rename(temporary, target)?;
    Ok(())
}

/// Resolve the asset index, asset objects, and library artifacts listed in Mojang's
/// version metadata. The launcher deliberately stores them in the standard game layout.
pub async fn install_runtime_files(version: &str) -> Result<()> {
    let root = game_dir()?;
    let metadata_path = root.join("versions").join(version).join(format!("{version}.json"));
    let raw = tokio::fs::read(&metadata_path).await
        .with_context(|| format!("missing metadata; run install {version} first"))?;
    let value: serde_json::Value = serde_json::from_slice(&raw)?;
    if let Some(index) = value.get("assetIndex") {
        let index: AssetIndexInfo = serde_json::from_value(index.clone())?;
        let index_path = root.join("assets").join("indexes").join(format!("{}.json", index.id));
        download_checked(&index.url, &index_path, Some(&index.sha1), Some(index.size)).await?;
        let bytes = tokio::fs::read(&index_path).await?;
        let parsed: serde_json::Value = serde_json::from_slice(&bytes)?;
        let objects: std::collections::HashMap<String, AssetObject> =
            serde_json::from_value(parsed.get("objects").cloned().context("asset index has no objects")?)?;
        let total = objects.len();
        for (n, object) in objects.values().enumerate() {
            let relative = format!("{}/{}", &object.hash[..2], object.hash);
            let target = root.join("assets").join("objects").join(relative);
            let url = format!("https://resources.download.minecraft.net/{}/{}", &object.hash[..2], object.hash);
            download_checked(&url, &target, Some(&object.hash), Some(object.size)).await
                .with_context(|| format!("asset {}/{}", n + 1, total))?;
        }
        println!("Assets ready: {} objects", total);
    }

    if let Some(libraries) = value.get("libraries").and_then(|v| v.as_array()) {
        let mut installed = 0usize;
        for entry in libraries {
            let library: LibraryInfo = serde_json::from_value(entry.clone())?;
            let downloads = library.downloads;
            if let Some(artifact) = downloads.as_ref().and_then(|d| d.artifact.as_ref()) {
                let relative = Path::new(&artifact.path);
                if relative.is_absolute() || relative.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
                    anyhow::bail!("unsafe library path in Mojang metadata");
                }
                let target = root.join("libraries").join(relative);
                download_checked(&artifact.url, &target, artifact.sha1.as_deref(), artifact.size).await?;
                installed += 1;
            }
            if let Some(native_key) = library.natives.as_ref().and_then(|n| n.get("linux")) {
                let classifier = native_key.replace("${arch}", "64");
                if let Some(native) = downloads.as_ref().and_then(|d| d.classifiers.as_ref()).and_then(|c| c.get(&classifier)) {
                    let relative = Path::new(&native.path);
                    if relative.is_absolute() || relative.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
                        anyhow::bail!("unsafe native library path in Mojang metadata");
                    }
                    let archive_path = root.join("libraries").join(relative);
                    download_checked(&native.url, &archive_path, native.sha1.as_deref(), native.size).await?;
                    extract_native_archive(&archive_path, &root.join("versions").join(version).join("natives"), library.extract.as_ref())?;
                    installed += 1;
                }
            }
        }
        println!("Libraries ready: {} artifacts", installed);
    }
    Ok(())
}

fn extract_native_archive(archive_path: &Path, destination: &Path, rules: Option<&NativeExtract>) -> Result<()> {
    let exclusions = rules.and_then(|r| r.exclude.as_ref());
    let file = std::fs::File::open(archive_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let name = entry.name().to_owned();
        if exclusions.map_or(false, |items| items.iter().any(|prefix| name.starts_with(prefix))) {
            continue;
        }
        let relative = Path::new(&name);
        if relative.is_absolute() || relative.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
            anyhow::bail!("unsafe path inside native archive: {name}");
        }
        let output = destination.join(relative);
        if entry.is_dir() {
            std::fs::create_dir_all(&output)?;
        } else {
            if let Some(parent) = output.parent() { std::fs::create_dir_all(parent)?; }
            let mut target = std::fs::File::create(output)?;
            std::io::copy(&mut entry, &mut target)?;
            target.sync_all()?;
        }
    }
    Ok(())
}

fn java_executable() -> Result<PathBuf> {
    if let Some(home) = std::env::var_os("JAVA_HOME") {
        let path = PathBuf::from(home).join("bin").join(if cfg!(windows) { "java.exe" } else { "java" });
        if path.is_file() { return Ok(path); }
    }
    Ok(PathBuf::from(if cfg!(windows) { "java.exe" } else { "java" }))
}

fn collect_jars(directory: &Path, output: &mut Vec<String>) -> Result<()> {
    if !directory.exists() { return Ok(()); }
    for entry in std::fs::read_dir(directory)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_jars(&path, output)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("jar") {
            output.push(path.to_string_lossy().into_owned());
        }
    }
    Ok(())
}

fn rule_matches(rule: &serde_json::Value) -> bool {
    let os = match rule.get("os") { Some(value) => value, None => return true };
    if let Some(name) = os.get("name").and_then(|v| v.as_str()) {
        if name != "linux" { return false; }
    }
    if let Some(arch) = os.get("arch").and_then(|v| v.as_str()) {
        if arch != "x86_64" && arch != "amd64" { return false; }
    }
    true
}

fn append_rule_argument(value: &serde_json::Value, output: &mut Vec<String>) {
    if let Some(text) = value.as_str() {
        output.push(text.to_owned());
    } else if let Some(array) = value.as_array() {
        for item in array { append_rule_argument(item, output); }
    } else if let Some(object) = value.as_object() {
        let rules = object.get("rules").and_then(|v| v.as_array());
        let allowed = if let Some(rules) = rules {
            let mut result = false;
            for rule in rules {
                if rule_matches(rule) {
                    result = rule.get("action").and_then(|v| v.as_str()) == Some("allow");
                }
            }
            result
        } else { true };
        if allowed {
            if let Some(inner) = object.get("value") { append_rule_argument(inner, output); }
        }
    }
}

/// Starts the installed client using Mojang's version argument metadata.
/// This is an offline profile bootstrap; Microsoft account authentication is not yet included.
pub fn launch_game(version: &str, username: &str) -> Result<()> {
    if username.trim().is_empty() { anyhow::bail!("username must not be empty"); }
    let root = game_dir()?;
    let version_dir = root.join("versions").join(version);
    let metadata_path = version_dir.join(format!("{version}.json"));
    let metadata: serde_json::Value = serde_json::from_slice(
        &std::fs::read(&metadata_path).with_context(|| format!("version {version} is not installed"))?
    )?;
    let client_jar = version_dir.join(format!("{version}.jar"));
    if !client_jar.is_file() { anyhow::bail!("client JAR is missing; run fpk install {version}"); }

    let mut classpath = vec![client_jar.to_string_lossy().into_owned()];
    collect_jars(&root.join("libraries"), &mut classpath)?;
    let classpath = classpath.join(if cfg!(windows) { ";" } else { ":" });
    let natives = version_dir.join("natives");
    let assets_root = root.join("assets");
    let asset_index = metadata.get("assetIndex").and_then(|v| v.get("id")).and_then(|v| v.as_str()).unwrap_or(version);
    let uuid = uuid::Uuid::new_v4().to_string();
    let replacements = [
        ("{natives_directory}", natives.to_string_lossy().as_ref()),
        ("{launcher_name}", "FPK"),
        ("{launcher_version}", "0.1.0"),
        ("{classpath}", classpath.as_str()),
        ("{classpath_separator}", if cfg!(windows) { ";" } else { ":" }),
        ("{auth_player_name}", username),
        ("{version_name}", version),
        ("{game_directory}", root.to_string_lossy().as_ref()),
        ("{assets_root}", assets_root.to_string_lossy().as_ref()),
        ("{assets_index_name}", asset_index),
        ("{auth_uuid}", uuid.as_str()),
        ("{auth_access_token}", "0"),
        ("{clientid}", "0"),
        ("{auth_xuid}", "0"),
        ("{user_type}", "legacy"),
        ("{version_type}", "release"),
        ("{resolution_width}", "1280"),
        ("{resolution_height}", "720"),
    ];
    let expand = |input: String| -> String {
        let mut result = input;
        for (key, value) in replacements { result = result.replace(key, value); }
        result
    };
    let mut jvm = Vec::new();
    if let Some(args) = metadata.get("arguments").and_then(|v| v.get("jvm")) {
        append_rule_argument(args, &mut jvm);
    } else {
        jvm.extend(["-Xmx2G".to_owned(), "-Djava.library.path={natives_directory}".to_owned()]);
    }
    let jvm: Vec<String> = jvm.into_iter().map(expand).collect();
    let mut game = Vec::new();
    if let Some(args) = metadata.get("arguments").and_then(|v| v.get("game")) {
        append_rule_argument(args, &mut game);
    } else if let Some(old) = metadata.get("minecraftArguments").and_then(|v| v.as_str()) {
        game.extend(old.split_whitespace().map(str::to_owned));
    }
    let game: Vec<String> = game.into_iter().map(expand).collect();
    let main_class = metadata.get("mainClass").and_then(|v| v.as_str()).context("version metadata has no mainClass")?;
    let java = java_executable()?;
    println!("Starting Minecraft {version} as offline profile '{username}'");
    let status = std::process::Command::new(java)
        .args(jvm)
        .arg(main_class)
        .args(game)
        .current_dir(&root)
        .status()
        .context("failed to start Java; install a compatible Java runtime and ensure it is on PATH")?;
    if !status.success() { anyhow::bail!("Minecraft process exited with {status}"); }
    Ok(())
}
