//! macOS `LaunchAgent` reconciliation for the background agent's autostart.
//!
//! When `Config::app_settings.launch_at_login` is `true`, a plist at
//! `~/Library/LaunchAgents/org.openlogi.agent.plist` is kept in sync with the
//! running agent executable so the next login relaunches it. `KeepAlive` is
//! `true` — the always-on daemon must survive a crash, the way Logi Options+'s
//! own agent does. No `--minimized`: the agent is always headless.
//!
//! The legacy `org.openlogi.openlogi` plist (the pre-split GUI autostart, which
//! launched the GUI with `--minimized`) is removed on every reconcile so the
//! GUI no longer self-launches. A still-running old instance is cleared at the
//! next logout.
//!
//! Production should register via `SMAppService` (so the entry shows in System
//! Settings → Login Items) once the app is signed + bundled with the plist in
//! `Contents/Library/LaunchAgents`; this hand-written plist is the unsigned /
//! dev path. TODO(signing): add the `SMAppService` registration path.

use tracing::debug;

#[cfg(target_os = "macos")]
use std::io;
#[cfg(target_os = "macos")]
use std::path::PathBuf;
#[cfg(target_os = "macos")]
use tracing::{info, warn};

/// Stable launch-agent identifier for the background agent.
#[cfg(target_os = "macos")]
const LABEL: &str = "org.openlogi.agent";

/// The pre-split GUI autostart label, removed on migration.
#[cfg(target_os = "macos")]
const LEGACY_LABEL: &str = "org.openlogi.openlogi";

/// Reconcile the agent's autostart with `enabled` and clear the legacy GUI
/// LaunchAgent. Idempotent; failures are logged, not propagated (startup must
/// not abort because `~/Library/LaunchAgents` is read-only).
pub fn reconcile(enabled: bool) {
    #[cfg(target_os = "macos")]
    {
        remove_legacy();
        if let Err(e) = reconcile_macos(enabled) {
            warn!(error = %e, enabled, "agent LaunchAgent reconcile failed");
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        if enabled {
            debug!("launch_at_login set but no autostart backend on this platform");
        }
        let _ = enabled;
    }
}

#[cfg(target_os = "macos")]
fn reconcile_macos(enabled: bool) -> io::Result<()> {
    let path = plist_path(LABEL)?;
    let exe = std::env::current_exe()?;
    let desired = enabled.then(|| render_plist(&exe.to_string_lossy()));

    let current = std::fs::read_to_string(&path).ok();
    match (desired.as_deref(), current.as_deref()) {
        (Some(want), Some(have)) if want == have => {
            debug!(path = %path.display(), "agent LaunchAgent already current");
        }
        (Some(want), _) => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, want)?;
            info!(path = %path.display(), "agent LaunchAgent installed");
        }
        (None, Some(_)) => {
            std::fs::remove_file(&path)?;
            info!(path = %path.display(), "agent LaunchAgent removed");
        }
        (None, None) => debug!("agent LaunchAgent already absent"),
    }
    Ok(())
}

/// Remove the legacy GUI LaunchAgent so the old `--minimized` GUI no longer
/// self-launches at login. Best-effort: a present-but-unreadable file is left
/// alone (logged), and a currently-running old instance survives until logout.
#[cfg(target_os = "macos")]
fn remove_legacy() {
    let Ok(path) = plist_path(LEGACY_LABEL) else {
        return;
    };
    if !path.exists() {
        return;
    }
    match std::fs::remove_file(&path) {
        Ok(()) => info!("removed legacy GUI LaunchAgent ({LEGACY_LABEL})"),
        Err(e) => warn!(error = %e, "could not remove legacy LaunchAgent"),
    }
}

#[cfg(target_os = "macos")]
fn plist_path(label: &str) -> io::Result<PathBuf> {
    let home = std::env::var_os("HOME")
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "$HOME not set"))?;
    Ok(PathBuf::from(home)
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{label}.plist")))
}

#[cfg(target_os = "macos")]
fn render_plist(exe: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
        <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \
        \"http://www.apple.com/DTD/PropertyList-1.0.dtd\">\n\
        <plist version=\"1.0\">\n\
        <dict>\n  \
        <key>Label</key>\n  \
        <string>{LABEL}</string>\n  \
        <key>ProgramArguments</key>\n  \
        <array>\n    \
        <string>{exe}</string>\n  \
        </array>\n  \
        <key>RunAtLoad</key>\n  \
        <true/>\n  \
        <key>KeepAlive</key>\n  \
        <true/>\n\
        </dict>\n\
        </plist>\n",
    )
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn rendered_plist_targets_the_agent_and_keeps_alive() {
        let body = render_plist(
            "/Applications/OpenLogi.app/Contents/Library/LoginItems/OpenLogiAgent.app/Contents/MacOS/openlogi-agent",
        );
        assert!(body.contains(LABEL));
        assert!(body.contains("openlogi-agent"));
        assert!(body.contains("RunAtLoad"));
        // KeepAlive must be true (the always-on daemon survives crashes) and
        // there is no --minimized arg (the agent is always headless).
        assert!(body.contains("<key>KeepAlive</key>\n  <true/>"));
        assert!(!body.contains("--minimized"));
    }
}
