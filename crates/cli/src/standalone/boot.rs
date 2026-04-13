use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use super::embed::read_payload_from_current_exe;
use super::format::{StandalonePayload, deserialize_payload_bytes, read_payload_from_exe};
use super::vfs::extract;

#[derive(Debug, Clone)]
pub enum LaunchMode {
    Dev {
        app_root: PathBuf,
        entry_path: PathBuf,
    },
    Standalone {
        app_root: PathBuf,
        entry_path: PathBuf,
        #[allow(dead_code)]
        extraction_dir: PathBuf,
    },
}

impl LaunchMode {
    pub fn app_root(&self) -> &Path {
        match self {
            LaunchMode::Dev { app_root, .. } => app_root,
            LaunchMode::Standalone { app_root, .. } => app_root,
        }
    }
    pub fn entry_path(&self) -> &Path {
        match self {
            LaunchMode::Dev { entry_path, .. } => entry_path,
            LaunchMode::Standalone { entry_path, .. } => entry_path,
        }
    }
}

/// Detect whether the current executable contains an embedded standalone
/// payload. If yes, extract it (idempotently) and return a Standalone launch
/// mode. Otherwise return `Ok(None)` so the caller can fall back to dev mode.
pub fn detect_and_prepare() -> Result<Option<LaunchMode>> {
    let exe = std::env::current_exe().context("resolving current_exe")?;
    let Some(payload) = load_payload(&exe)? else {
        return Ok(None);
    };

    let extraction_dir = choose_extraction_dir(&exe, &payload)?;
    ensure_extracted(&payload, &extraction_dir)?;

    let app_root = extraction_dir.join(&payload.metadata.dist_root_dir_name);
    // `entry_rel_path` already includes the dist_root_dir_name as prefix
    // (e.g. "app/index.js"); resolve against extraction_dir for a stable path.
    let entry_path = extraction_dir.join(
        payload
            .metadata
            .entry_rel_path
            .replace('/', std::path::MAIN_SEPARATOR_STR),
    );

    Ok(Some(LaunchMode::Standalone {
        app_root,
        entry_path,
        extraction_dir,
    }))
}

/// Resolve the embedded payload for the current executable.
///
/// Tries the production path first — a real PE resource / Mach-O section /
/// ELF note section read via libsui — and falls back to the legacy v1
/// trailer-append format so existing packed binaries keep booting during
/// the transition.
fn load_payload(exe: &Path) -> Result<Option<StandalonePayload>> {
    match read_payload_from_current_exe() {
        Ok(Some(bytes)) => match deserialize_payload_bytes(&bytes) {
            Ok(payload) => return Ok(Some(payload)),
            Err(e) => {
                eprintln!(
                    "uzumaki: native section present but failed to deserialize: {e}; \
                     falling back to legacy trailer reader"
                );
            }
        },
        Ok(None) => {}
        Err(e) => {
            eprintln!(
                "uzumaki: native section lookup failed: {e}; \
                 falling back to legacy trailer reader"
            );
        }
    }

    read_payload_from_exe(exe)
}

fn choose_extraction_dir(exe: &Path, payload: &StandalonePayload) -> Result<PathBuf> {
    let hash = &payload.metadata.extract_hash;
    let exe_stem = exe
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("uzumaki_app");

    // Try next-to-exe first: ./.<exe>/<hash>/
    if let Some(parent) = exe.parent() {
        let dir = parent.join(format!(".{}", exe_stem)).join(hash);
        if can_use_dir(&dir) {
            return Ok(dir);
        }
    }

    // Fall back to a platform-appropriate local data directory.
    let base = local_data_dir().unwrap_or_else(std::env::temp_dir);
    Ok(base.join("uzumaki").join(exe_stem).join(hash))
}

fn can_use_dir(path: &Path) -> bool {
    if let Err(e) = fs::create_dir_all(path) {
        eprintln!(
            "uzumaki: cannot create extraction dir {}: {}",
            path.display(),
            e
        );
        return false;
    }
    // Probe writability by creating and removing a tiny file.
    let probe = path.join(".probe");
    match fs::write(&probe, b"") {
        Ok(()) => {
            let _ = fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

fn local_data_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        if let Ok(v) = std::env::var("LOCALAPPDATA") {
            return Some(PathBuf::from(v));
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            return Some(
                PathBuf::from(home)
                    .join("Library")
                    .join("Application Support"),
            );
        }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Ok(v) = std::env::var("XDG_DATA_HOME") {
            return Some(PathBuf::from(v));
        }
        if let Ok(home) = std::env::var("HOME") {
            return Some(PathBuf::from(home).join(".local").join("share"));
        }
    }
    None
}

fn ensure_extracted(payload: &StandalonePayload, extraction_dir: &Path) -> Result<()> {
    let done_marker = extraction_dir.join(".done");
    if done_marker.exists() {
        return Ok(());
    }

    fs::create_dir_all(extraction_dir)
        .with_context(|| format!("creating {}", extraction_dir.display()))?;
    extract(payload, extraction_dir)?;
    fs::write(&done_marker, b"1")
        .with_context(|| format!("writing done marker {}", done_marker.display()))?;
    Ok(())
}
