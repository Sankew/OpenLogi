use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

use anyhow::{Context as _, Result, bail};

pub(crate) fn repo_root() -> Result<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .context("could not locate repository root from xtask manifest directory")
}

pub(crate) fn run(command: &mut ProcessCommand) -> Result<()> {
    let status = command
        .status()
        .with_context(|| format!("could not run {}", display_command(command)))?;
    if status.success() {
        Ok(())
    } else {
        bail!("{} failed with {status}", display_command(command));
    }
}

pub(crate) fn command_stdout(command: &mut ProcessCommand) -> Result<String> {
    let output = command
        .output()
        .with_context(|| format!("could not run {}", display_command(command)))?;
    if !output.status.success() {
        bail!("{} failed with {}", display_command(command), output.status);
    }
    String::from_utf8(output.stdout).context("command output was not valid UTF-8")
}

fn display_command(command: &ProcessCommand) -> String {
    let mut text = command.get_program().to_string_lossy().into_owned();
    for arg in command.get_args() {
        text.push(' ');
        text.push_str(&shell_quote(arg));
    }
    text
}

fn shell_quote(arg: &OsStr) -> String {
    let text = arg.to_string_lossy();
    if text
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '_' | '-' | ':' | '='))
    {
        text.into_owned()
    } else {
        format!("'{escaped}'", escaped = text.replace('\'', "'\\''"))
    }
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
