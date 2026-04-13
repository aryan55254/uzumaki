use anyhow::{Context, Result, anyhow};
use std::fs;
use std::path::{Path, PathBuf};

use super::format::{StandalonePayload, VfsEntry};

/// Walk a directory recursively and return (relative_path, absolute_path) pairs.
/// Relative paths use forward slashes so they are stable across platforms.
pub fn walk_dir(root: &Path) -> Result<Vec<(String, PathBuf)>> {
    let mut out = Vec::new();
    walk_inner(root, root, &mut out)?;
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

fn walk_inner(root: &Path, cur: &Path, out: &mut Vec<(String, PathBuf)>) -> Result<()> {
    for entry in fs::read_dir(cur).with_context(|| format!("reading {}", cur.display()))? {
        let entry = entry?;
        let path = entry.path();
        let ft = entry.file_type()?;
        if ft.is_dir() {
            walk_inner(root, &path, out)?;
        } else if ft.is_file() {
            let rel = path
                .strip_prefix(root)
                .map_err(|_| anyhow!("strip_prefix failed for {}", path.display()))?;
            let rel_str = rel
                .to_str()
                .ok_or_else(|| anyhow!("non-utf8 path: {}", rel.display()))?
                .replace('\\', "/");
            out.push((rel_str, path));
        }
    }
    Ok(())
}

/// Extract the payload into `target_root` so that
/// `target_root/<dist_root_dir_name>/<relative_path>` contains each file.
pub fn extract(payload: &StandalonePayload, target_root: &Path) -> Result<()> {
    let dist_root = target_root.join(&payload.metadata.dist_root_dir_name);
    fs::create_dir_all(&dist_root).with_context(|| format!("creating {}", dist_root.display()))?;

    for entry in &payload.manifest {
        write_entry(payload, &dist_root, entry)?;
    }
    Ok(())
}

fn write_entry(payload: &StandalonePayload, dist_root: &Path, entry: &VfsEntry) -> Result<()> {
    let target = dist_root.join(
        entry
            .relative_path
            .replace('/', std::path::MAIN_SEPARATOR_STR),
    );
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    let start = entry.offset as usize;
    let end = start
        .checked_add(entry.len as usize)
        .ok_or_else(|| anyhow!("manifest entry length overflow"))?;
    if end > payload.blob.len() {
        return Err(anyhow!(
            "manifest entry out of bounds for {}",
            entry.relative_path
        ));
    }
    fs::write(&target, &payload.blob[start..end])
        .with_context(|| format!("writing {}", target.display()))?;
    Ok(())
}
