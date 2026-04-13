use anyhow::{Context, Result, bail};
use clap::{CommandFactory, Parser, Subcommand};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::standalone;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const GITHUB_REPO: &str = "golok727/uzumaki";

// ─── Config ────────────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
pub struct UzumakiConfig {
    #[serde(default)]
    pub build: BuildConfig,
    #[serde(default)]
    pub pack: PackConfig,
}

#[derive(Debug, Default, serde::Deserialize)]
pub struct BuildConfig {
    pub command: Option<String>,
}

#[derive(Debug, Default, serde::Deserialize)]
pub struct PackConfig {
    pub dist: Option<String>,
    pub entry: Option<String>,
    pub output: Option<String>,
    pub name: Option<String>,
    #[serde(rename = "baseBinary")]
    pub base_binary: Option<String>,
}

fn find_config(start: &Path) -> Option<PathBuf> {
    let mut dir = start.to_path_buf();
    loop {
        let candidate = dir.join("uzumaki.config.json");
        if candidate.is_file() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

fn load_config(path: &Path) -> Result<UzumakiConfig> {
    let raw = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&raw).with_context(|| format!("parsing {}", path.display()))
}

// ─── CLI ───────────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name = "uzumaki",
    about = "\x1b[1;38;5;75mUzumaki\x1b[0m — Desktop UI Framework",
    version = VERSION,
    styles = clap_styles(),
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Run a JS/TS file in the uzumaki runtime
    Run {
        /// Entry point file
        entry: String,
        /// Extra arguments passed to the runtime
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Build and package an app using uzumaki.config.json
    Build {
        /// Path to config file
        #[arg(long)]
        config: Option<String>,
        /// Skip the build step
        #[arg(long)]
        no_build: bool,
    },
    /// Low-level: pack a dist folder into a standalone executable
    Pack {
        /// Directory containing built JS files
        #[arg(long)]
        dist: String,
        /// Relative entry point inside dist
        #[arg(long)]
        entry: String,
        /// Output executable path
        #[arg(long, short)]
        output: String,
        /// Application name
        #[arg(long)]
        name: Option<String>,
        /// Base binary to embed into
        #[arg(long)]
        base_binary: Option<String>,
    },
    /// Upgrade uzumaki to the latest version
    Upgrade {
        /// Specific version to install (e.g. 0.2.0)
        #[arg(long)]
        version: Option<String>,
    },
}

fn clap_styles() -> clap::builder::Styles {
    clap::builder::Styles::styled()
        .header(
            clap::builder::styling::AnsiColor::BrightCyan
                .on_default()
                .bold(),
        )
        .usage(
            clap::builder::styling::AnsiColor::BrightCyan
                .on_default()
                .bold(),
        )
        .literal(clap::builder::styling::AnsiColor::BrightBlue.on_default())
        .placeholder(clap::builder::styling::AnsiColor::White.on_default())
}

// ─── Command implementations ───────────────────────────────────────────────

/// Known subcommand names so we can distinguish `uzumaki build` from `uzumaki app.tsx`.
const KNOWN_SUBCOMMANDS: &[&str] = &["run", "build", "pack", "upgrade", "help"];

pub fn run_cli() -> Result<Option<standalone::LaunchMode>> {
    let raw_args: Vec<String> = std::env::args().collect();

    // No args → print help and exit successfully
    if raw_args.len() <= 1 {
        Cli::command().print_help().ok();
        println!();
        return Ok(None);
    }

    // If the first arg after the binary name looks like a file (not a known subcommand),
    // treat it as `uzumaki run <file> ...` — same as bun/deno.
    let cli = if !KNOWN_SUBCOMMANDS.contains(&raw_args[1].as_str()) && !raw_args[1].starts_with('-')
    {
        let mut patched = vec![raw_args[0].clone(), "run".to_string()];
        patched.extend_from_slice(&raw_args[1..]);
        Cli::parse_from(patched)
    } else {
        Cli::parse()
    };

    match cli.command {
        Commands::Run { entry, args: _ } => Ok(Some(resolve_run(&entry)?)),
        Commands::Build { config, no_build } => {
            cmd_build(config.as_deref(), no_build)?;
            Ok(None)
        }
        Commands::Pack {
            dist,
            entry,
            output,
            name,
            base_binary,
        } => {
            cmd_pack(
                &dist,
                &entry,
                &output,
                name.as_deref(),
                base_binary.as_deref(),
            )?;
            Ok(None)
        }
        Commands::Upgrade { version } => {
            cmd_update(version.as_deref())?;
            Ok(None)
        }
    }
}

fn resolve_run(entry: &str) -> Result<standalone::LaunchMode> {
    let cwd = std::env::current_dir()?;
    let entry_path = fs::canonicalize(cwd.join(entry))
        .with_context(|| format!("entry point not found: {entry}"))?;
    let app_root = entry_path.parent().map(|p| p.to_path_buf()).unwrap_or(cwd);
    Ok(standalone::LaunchMode::Dev {
        app_root,
        entry_path,
    })
}

// ─── build ─────────────────────────────────────────────────────────────────

fn cmd_build(config_path: Option<&str>, no_build: bool) -> Result<()> {
    let cwd = std::env::current_dir()?;

    let config_file = match config_path {
        Some(p) => {
            let p = cwd.join(p);
            if !p.is_file() {
                bail!("config file not found: {}", p.display());
            }
            p
        }
        None => find_config(&cwd).ok_or_else(|| {
            anyhow::anyhow!("could not find uzumaki.config.json from {}", cwd.display())
        })?,
    };

    let config_dir = config_file.parent().unwrap().to_path_buf();
    let config = load_config(&config_file)?;

    // Run build command
    if !no_build && let Some(ref cmd) = config.build.command {
        println!(
            "\x1b[1;38;5;75muzumaki\x1b[0m \x1b[2mrunning build:\x1b[0m {}",
            cmd
        );
        let status = run_shell_command(cmd, &config_dir)?;
        if !status.success() {
            bail!("build command failed with exit code {}", status);
        }
    }

    // Pack
    let dist = config
        .pack
        .dist
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("missing pack.dist in config"))?;
    let entry = config
        .pack
        .entry
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("missing pack.entry in config"))?;
    let output_raw = config
        .pack
        .output
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("missing pack.output in config"))?;

    let dist_path = resolve_from(&config_dir, dist);
    let output_path = normalize_output_extension(&resolve_from(&config_dir, output_raw));
    let app_name = config.pack.name.clone().unwrap_or_else(|| {
        output_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("uzumaki-app")
            .to_string()
    });
    let base_binary = match &config.pack.base_binary {
        Some(b) => resolve_from(&config_dir, b),
        None => std::env::current_exe()?,
    };

    println!(
        "\x1b[1;38;5;75muzumaki\x1b[0m \x1b[2mpacking\x1b[0m {} → {}",
        dist,
        output_path.display()
    );

    standalone::pack::pack_app(&standalone::pack::PackOptions {
        dist_dir: dist_path,
        entry_rel: entry.to_string(),
        output: output_path,
        app_name,
        base_binary,
    })
}

fn cmd_pack(
    dist: &str,
    entry: &str,
    output: &str,
    name: Option<&str>,
    base_binary: Option<&str>,
) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let dist_path = resolve_from(&cwd, dist);
    let output_path = normalize_output_extension(&resolve_from(&cwd, output));
    let base = match base_binary {
        Some(b) => resolve_from(&cwd, b),
        None => std::env::current_exe()?,
    };
    let app_name = name.map(|s| s.to_string()).unwrap_or_else(|| {
        output_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("uzumaki-app")
            .to_string()
    });

    standalone::pack::pack_app(&standalone::pack::PackOptions {
        dist_dir: dist_path,
        entry_rel: entry.to_string(),
        output: output_path,
        app_name,
        base_binary: base,
    })
}

// ─── update ────────────────────────────────────────────────────────────────

fn cmd_update(target_version: Option<&str>) -> Result<()> {
    println!("\x1b[1;38;5;75muzumaki\x1b[0m \x1b[2mchecking for updates...\x1b[0m");

    let version_tag = match target_version {
        Some(v) => {
            if v.starts_with('v') {
                v.to_string()
            } else {
                format!("v{v}")
            }
        }
        None => {
            // Fetch latest release tag from GitHub API
            let url = format!("https://api.github.com/repos/{GITHUB_REPO}/releases/latest");
            let body: String = ureq::get(&url)
                .header("Accept", "application/vnd.github+json")
                .header("User-Agent", "uzumaki-updater")
                .call()
                .context("failed to fetch latest release")?
                .body_mut()
                .read_to_string()
                .context("failed to read response body")?;
            let release: serde_json::Value =
                serde_json::from_str(&body).context("invalid JSON from GitHub API")?;
            release["tag_name"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("no tag_name in latest release"))?
                .to_string()
        }
    };

    let version_num = version_tag.strip_prefix('v').unwrap_or(&version_tag);

    if version_num == VERSION {
        println!("\x1b[1;38;5;75muzumaki\x1b[0m \x1b[32malready up to date\x1b[0m (v{VERSION})");
        return Ok(());
    }

    let asset_name = get_asset_name();
    let download_url =
        format!("https://github.com/{GITHUB_REPO}/releases/download/{version_tag}/{asset_name}");

    println!("\x1b[1;38;5;75muzumaki\x1b[0m \x1b[2mdownloading\x1b[0m v{VERSION} → v{version_num}");

    let mut response = ureq::get(&download_url)
        .header("User-Agent", "uzumaki-updater")
        .call()
        .with_context(|| format!("failed to download {download_url}"))?;

    let body_bytes = response
        .body_mut()
        .read_to_vec()
        .context("failed to read download body")?;

    // The asset is a zip file containing the binary
    let binary_bytes = extract_binary_from_zip(&body_bytes, &get_binary_name())?;

    // Replace the current executable
    let current_exe = std::env::current_exe()?;
    replace_exe(&current_exe, &binary_bytes)?;

    println!("\x1b[1;38;5;75muzumaki\x1b[0m \x1b[32mupdated to v{version_num}\x1b[0m");

    Ok(())
}

fn extract_binary_from_zip(zip_bytes: &[u8], binary_name: &str) -> Result<Vec<u8>> {
    let reader = std::io::Cursor::new(zip_bytes);
    let mut archive = zip::ZipArchive::new(reader).context("invalid zip archive")?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().to_string();
        if name == binary_name || name.ends_with(&format!("/{binary_name}")) {
            let mut bytes = Vec::with_capacity(file.size() as usize);
            std::io::Read::read_to_end(&mut file, &mut bytes)?;
            return Ok(bytes);
        }
    }

    bail!("binary '{binary_name}' not found in zip archive")
}

fn replace_exe(current_exe: &Path, new_bytes: &[u8]) -> Result<()> {
    let dir = current_exe.parent().unwrap();
    let tmp_file = tempfile::NamedTempFile::new_in(dir)?;
    fs::write(tmp_file.path(), new_bytes)?;

    // On Unix, set executable permission
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(tmp_file.path(), fs::Permissions::from_mode(0o755))?;
    }

    // Rename strategy: move old away, move new in
    let backup_path = current_exe.with_extension("old");
    // Remove leftover backup from a previous update
    let _ = fs::remove_file(&backup_path);

    // On Windows we can't overwrite a running exe, but we CAN rename it away
    fs::rename(current_exe, &backup_path)
        .with_context(|| format!("failed to move current exe to {}", backup_path.display()))?;

    if let Err(e) = fs::rename(tmp_file.path(), current_exe) {
        // Rollback
        let _ = fs::rename(&backup_path, current_exe);
        return Err(e).context("failed to place new binary");
    }

    // Keep the temp file from being deleted since we already moved it
    tmp_file.into_temp_path().keep()?;

    // Clean up backup
    let _ = fs::remove_file(&backup_path);

    Ok(())
}

// ─── Helpers ───────────────────────────────────────────────────────────────

fn resolve_from(base: &Path, value: &str) -> PathBuf {
    let p = Path::new(value);
    let joined = if p.is_absolute() {
        p.to_path_buf()
    } else {
        base.join(p)
    };
    normalize_path(&joined)
}

/// Lexically normalize a path — resolves `.` and `..` without requiring the path to exist.
fn normalize_path(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

fn normalize_output_extension(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if cfg!(target_os = "windows") {
        if s.ends_with(".exe") {
            path.to_path_buf()
        } else {
            PathBuf::from(format!("{s}.exe"))
        }
    } else if cfg!(target_os = "macos") {
        // Strip wrong extensions, no extension needed on macOS
        let cleaned = s.trim_end_matches(".exe").trim_end_matches(".app");
        PathBuf::from(cleaned.to_string())
    } else {
        // Linux – strip .exe if present
        let cleaned = s.trim_end_matches(".exe");
        PathBuf::from(cleaned.to_string())
    }
}

fn run_shell_command(command: &str, cwd: &Path) -> Result<std::process::ExitStatus> {
    let status = if cfg!(target_os = "windows") {
        Command::new("cmd.exe")
            .args(["/d", "/s", "/c", command])
            .current_dir(cwd)
            .status()
    } else {
        Command::new("sh")
            .args(["-lc", command])
            .current_dir(cwd)
            .status()
    };
    status.with_context(|| format!("failed to run: {command}"))
}

fn get_binary_name() -> String {
    if cfg!(target_os = "windows") {
        "uzumaki.exe".to_string()
    } else {
        "uzumaki".to_string()
    }
}

fn get_asset_name() -> String {
    let os = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    };

    let arch = if cfg!(target_arch = "x86_64") {
        "x64"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "x64"
    };

    format!("uzumaki-{os}-{arch}.zip")
}
