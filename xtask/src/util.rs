use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

use anyhow::{Context as _, Result, bail};
use command_error::CommandExt;

pub(crate) fn repo_root() -> Result<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .context("could not locate repository root from xtask manifest directory")
}

pub(crate) fn run(command: &mut ProcessCommand) -> Result<()> {
    command.status_checked()?;
    Ok(())
}

pub(crate) fn command_stdout(command: &mut ProcessCommand) -> Result<String> {
    Ok(command.output_checked_utf8()?.stdout)
}

pub(crate) fn with_env<'a>(
    command: &'a mut ProcessCommand,
    vars: &[(String, String)],
) -> &'a mut ProcessCommand {
    for (key, value) in vars {
        command.env(key, value);
    }
    command
}

pub(crate) fn command_exists(name: &str) -> bool {
    which::which(name).is_ok()
}

pub(crate) fn ensure_command(name: &str) -> Result<()> {
    if command_exists(name) {
        Ok(())
    } else {
        bail!("{name} not found — run inside the devenv shell, or install it first")
    }
}

pub(crate) fn ensure_file(path: &Path) -> Result<()> {
    if path.is_file() {
        Ok(())
    } else {
        bail!("missing file {}", path.display())
    }
}

pub(crate) fn ensure_dir(path: &Path) -> Result<()> {
    if path.is_dir() {
        Ok(())
    } else {
        bail!("missing directory {}", path.display())
    }
}

pub(crate) fn absolutize(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}
