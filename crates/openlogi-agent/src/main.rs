//! OpenLogi background agent — headless, always-on.
//!
//! Owns the CGEventTap hook and the HID++ device path (gesture capture, DPI,
//! SmartShift), driven by `config.toml` and live device inventory. Phase 1: no
//! IPC — it reads the config once at startup and applies it; the GUI talks to it
//! over IPC in a later phase, which is also where live config reload lands.

mod launch_agent;
mod server;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use openlogi_agent_core::orchestrator::Orchestrator;
use openlogi_agent_core::{hook_runtime, watchers};
use openlogi_core::config::Config;
use openlogi_hook::Hook;
use tokio::sync::Mutex;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

use crate::server::AgentServer;

fn main() {
    init_tracing();

    let config = Config::load_or_default().unwrap_or_else(|e| {
        warn!(error = %e, "could not load config.toml; using defaults");
        Config::default()
    });

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            warn!(error = %e, "tokio runtime init failed; agent exiting");
            return;
        }
    };
    runtime.block_on(run(config));
}

async fn run(config: Config) {
    // Reconcile the agent's launch-at-login autostart and clear the legacy GUI
    // LaunchAgent, before `config` moves into the orchestrator.
    launch_agent::reconcile(config.app_settings.launch_at_login);

    // The orchestrator is shared with the IPC server (which serves inventory /
    // reload / status) and mutated by the watcher select loop, so it lives
    // behind an async mutex. Locks are brief (a map rebuild or a clone).
    let orchestrator = Arc::new(Mutex::new(Orchestrator::new(config)));
    let shared = orchestrator.lock().await.shared();
    let hook_installed = Arc::new(AtomicBool::new(false));

    // The HID++ control watcher (gesture button, DPI/ModeShift button, thumb
    // wheel) needs no Accessibility permission — start it up front. It reads the
    // shared maps and dispatches bound actions itself.
    watchers::gesture::spawn(
        shared.hook_bindings.clone(),
        shared.gesture_bindings.clone(),
        shared.dpi_cycle.clone(),
        shared.capture_channel.clone(),
        shared.thumbwheel_sensitivity.clone(),
    );

    let mut inventory_rx = watchers::inventory::spawn(Duration::from_secs(2));
    let mut app_rx = watchers::foreground_app::spawn(Duration::from_secs(1));
    let mut accessibility_rx = watchers::accessibility::spawn(Duration::from_millis(1200));

    // IPC server: the GUI connects here for device state + "apply now" commands.
    match openlogi_core::paths::agent_socket_path() {
        Ok(socket_path) => {
            let server = AgentServer {
                orchestrator: Arc::clone(&orchestrator),
                shared: shared.clone(),
                hook_installed: Arc::clone(&hook_installed),
            };
            tokio::spawn(server::run(server, socket_path));
        }
        Err(e) => warn!(error = %e, "could not resolve IPC socket path; IPC disabled"),
    }

    // The CGEventTap hook is installed once Accessibility is granted and dropped
    // if it's revoked (the tap self-disables on revoke regardless; dropping the
    // handle stops its thread).
    let mut hook: Option<Hook> = None;

    info!("openlogi-agent started");
    loop {
        tokio::select! {
            Some(inventories) = inventory_rx.recv() => {
                orchestrator.lock().await.refresh_inventory(&inventories);
            }
            Some(bundle) = app_rx.recv() => {
                orchestrator.lock().await.set_current_app(bundle);
            }
            Some(granted) = accessibility_rx.recv() => {
                if !granted {
                    hook = None;
                    hook_installed.store(false, Ordering::Relaxed);
                }
                if granted && hook.is_none() {
                    info!("accessibility granted — installing OS mouse hook");
                    hook = hook_runtime::start(
                        shared.hook_bindings.clone(),
                        shared.dpi_cycle.clone(),
                        shared.capture_channel.clone(),
                    );
                    hook_installed.store(hook.is_some(), Ordering::Relaxed);
                }
            }
            else => break,
        }
    }
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            EnvFilter::try_from_env("OPENLOGI_LOG").unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();
}
