//! OS input-event synthesis for each [`Action`], split out of openlogi-core so
//! the core schema stays platform- and IO-free.
//!
//! [`execute`] is the single entry point: it dispatches to the per-platform
//! synthesiser ([`execute_macos`]/[`execute_linux`]/[`execute_windows`]), each of
//! which translates an [`Action`] into the native event(s) — CGEvent/NSEvent on
//! macOS, uinput/D-Bus on Linux, SendInput on Windows.

use openlogi_core::binding::Action;

/// Synthesise the OS-level event for `action`.
///
/// On macOS, key events are posted via `CGEventPost(kCGHIDEventTap, …)`
/// using virtual key codes from the standard US keyboard layout, and the
/// `LeftClick`/`RightClick`/`MiddleClick` variants synthesise a mouse click
/// at the current cursor location. The WindowServer actions (`MissionControl`,
/// `AppExpose`, `ShowDesktop`, `LaunchpadShow`) are posted straight to the
/// Dock via `CoreDockSendNotification`. Device-side actions (`CycleDpiPresets`,
/// `SetDpiPreset`, `ToggleSmartShift`) have no CGEvent equivalent and are
/// handled at the hook/HID layer, logging a trace here.
///
/// On Linux, key and scroll events are injected via a lazily-created `uinput`
/// virtual device. Mouse clicks inject `BTN_*` events. macOS-only window
/// manager actions (`MissionControl`, `AppExpose`, `ShowDesktop`,
/// `LaunchpadShow`) have no universal Linux equivalent and are silently
/// skipped (debug-logged). `CustomShortcut` maps macOS `kVK_*` codes to
/// Linux key codes; macOS Cmd maps to Ctrl.
///
/// On Windows, key and mouse events are synthesised via `SendInput`. The
/// macOS window-manager actions map to their Windows equivalents (e.g.
/// `MissionControl` → Win+Tab, `ShowDesktop` → Win+D); `CustomShortcut`
/// maps macOS `kVK_*` codes to Windows virtual-key codes, with Cmd mapped to
/// Ctrl.
///
/// On other platforms a warning is logged and the function returns
/// immediately — the binary compiles clean on all targets.
///
/// # Manual verification
///
/// `execute` is intentionally excluded from the automated test suite because
/// it would need to intercept the OS event queue. Smoke-test it manually:
/// bind a button to any action in the GUI and confirm the expected system event
/// fires when the button is pressed (or use the `inject_action` example).
pub fn execute(action: &Action) {
    #[cfg(target_os = "macos")]
    execute_macos(action);

    #[cfg(target_os = "linux")]
    execute_linux(action);

    #[cfg(target_os = "windows")]
    execute_windows(action);

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        tracing::warn!(
            action = action.label(),
            "execute unsupported on this platform"
        );
    }
}

/// Linux implementation: inject events via a shared `uinput` virtual device.
#[cfg(target_os = "linux")]
fn execute_linux(action: &Action) {
    use evdev::{KeyCode, RelativeAxisCode};
    let ctrl = KeyCode::KEY_LEFTCTRL;
    let shift = KeyCode::KEY_LEFTSHIFT;
    let alt = KeyCode::KEY_LEFTALT;
    match action {
        // ── Mouse clicks ──────────────────────────────────────────────────
        Action::LeftClick => linux::click(KeyCode::BTN_LEFT),
        Action::RightClick => linux::click(KeyCode::BTN_RIGHT),
        Action::MiddleClick => linux::click(KeyCode::BTN_MIDDLE),
        // Extra mouse buttons: BTN_SIDE/BTN_EXTRA are the evdev side
        // buttons ("back"/"forward") browsers handle natively.
        Action::MouseBack => linux::click(KeyCode::BTN_SIDE),
        Action::MouseForward => linux::click(KeyCode::BTN_EXTRA),
        // ── Editing ───────────────────────────────────────────────────────
        Action::Copy => linux::press_key(&[ctrl], KeyCode::KEY_C),
        Action::Paste => linux::press_key(&[ctrl], KeyCode::KEY_V),
        Action::Cut => linux::press_key(&[ctrl], KeyCode::KEY_X),
        Action::Undo => linux::press_key(&[ctrl], KeyCode::KEY_Z),
        // Redo is Ctrl+Shift+Z on Linux (matches macOS ⌘⇧Z convention).
        Action::Redo => linux::press_key(&[ctrl, shift], KeyCode::KEY_Z),
        Action::SelectAll => linux::press_key(&[ctrl], KeyCode::KEY_A),
        Action::Find => linux::press_key(&[ctrl], KeyCode::KEY_F),
        Action::Save => linux::press_key(&[ctrl], KeyCode::KEY_S),
        // ── Browser / Navigation ──────────────────────────────────────────
        Action::BrowserBack => linux::press_key(&[alt], KeyCode::KEY_LEFT),
        Action::BrowserForward => linux::press_key(&[alt], KeyCode::KEY_RIGHT),
        Action::NewTab => linux::press_key(&[ctrl], KeyCode::KEY_T),
        Action::CloseTab => linux::press_key(&[ctrl], KeyCode::KEY_W),
        Action::ReopenTab => linux::press_key(&[ctrl, shift], KeyCode::KEY_T),
        Action::NextTab => linux::press_key(&[ctrl], KeyCode::KEY_TAB),
        Action::PrevTab => linux::press_key(&[ctrl, shift], KeyCode::KEY_TAB),
        Action::ReloadPage => linux::press_key(&[ctrl], KeyCode::KEY_R),
        // ── Navigation — macOS-specific ───────────────────────────────────
        // No universal Linux equivalent; the compositor shortcut varies.
        Action::MissionControl
        | Action::AppExpose
        | Action::ShowDesktop
        | Action::LaunchpadShow => {
            tracing::debug!(
                action = action.label(),
                "no Linux equivalent — action skipped"
            );
        }
        // Ctrl+Alt+←/→ is the default in GNOME and KDE.
        Action::PreviousDesktop => linux::press_key(&[ctrl, alt], KeyCode::KEY_LEFT),
        Action::NextDesktop => linux::press_key(&[ctrl, alt], KeyCode::KEY_RIGHT),
        // ── System ────────────────────────────────────────────────────────
        // logind LockSessions() via the system bus; falls back to Super+L.
        Action::LockScreen => linux::lock_screen(),
        // Region vs full-screen capture depends on the desktop environment's
        // screenshot handler for Print Screen, so both map to the same key.
        Action::Screenshot | Action::CaptureRegion => linux::press_key(&[], KeyCode::KEY_SYSRQ),
        // ── Media ─────────────────────────────────────────────────────────
        // MPRIS targets the running media player; XF86 volume keys go to the
        // system mixer (PulseAudio/PipeWire) which is what users expect.
        Action::PlayPause => linux::mpris_command("PlayPause"),
        Action::NextTrack => linux::mpris_command("Next"),
        Action::PrevTrack => linux::mpris_command("Previous"),
        Action::VolumeUp => linux::press_key(&[], KeyCode::KEY_VOLUMEUP),
        Action::VolumeDown => linux::press_key(&[], KeyCode::KEY_VOLUMEDOWN),
        Action::MuteVolume => linux::press_key(&[], KeyCode::KEY_MUTE),
        // ── DPI / SmartShift: handled at hook/HID layer ───────────────────
        Action::CycleDpiPresets | Action::SetDpiPreset(_) | Action::ToggleSmartShift => {
            tracing::debug!(
                action = action.label(),
                "device action handled by hook/HID layer"
            );
        }
        // ── Scroll ────────────────────────────────────────────────────────
        Action::ScrollUp => linux::scroll(RelativeAxisCode::REL_WHEEL, 3),
        Action::ScrollDown => linux::scroll(RelativeAxisCode::REL_WHEEL, -3),
        Action::HorizontalScrollLeft => linux::scroll(RelativeAxisCode::REL_HWHEEL, -3),
        Action::HorizontalScrollRight => linux::scroll(RelativeAxisCode::REL_HWHEEL, 3),
        // ── No-op ─────────────────────────────────────────────────────────
        Action::None => {}
        // ── Custom shortcut ───────────────────────────────────────────────
        Action::CustomShortcut(combo) => {
            if combo.key_code == 0 {
                tracing::warn!(
                    chord = %combo.rendered_label(),
                    "CustomShortcut with no key code — press ignored"
                );
                return;
            }
            let Some(key) = linux::macos_vk_to_linux(combo.key_code) else {
                tracing::warn!(
                    key_code = combo.key_code,
                    "CustomShortcut key code has no Linux mapping — press ignored"
                );
                return;
            };
            linux::press_key(&linux::modifiers_to_keycodes(combo.modifiers), key);
        }
    }
}

/// macOS implementation: dispatch to the appropriate event helper.
#[cfg(target_os = "macos")]
fn execute_macos(action: &Action) {
    use core_graphics::event::{CGEventFlags, CGMouseButton};
    use openlogi_core::binding::KeyCombo;

    // Modifier bit shorthands.
    let cmd = CGEventFlags::CGEventFlagCommand;
    let shift = CGEventFlags::CGEventFlagShift;
    let ctrl = CGEventFlags::CGEventFlagControl;

    match action {
        // Suppressed input: captured but deliberately produces no event.
        Action::None => {}
        // ── Mouse clicks: synthesise a click at the cursor ────────────────
        // Remapping a *different* button to a click lands here (e.g. Back →
        // MiddleClick). A button left on its own native click never reaches
        // this — the hook passes it straight through to the OS.
        Action::LeftClick => macos::post_click(CGMouseButton::Left),
        Action::RightClick => macos::post_click(CGMouseButton::Right),
        Action::MiddleClick => macos::post_click(CGMouseButton::Center),
        // Extra mouse buttons: post the real button4/5 the OS treats as
        // back/forward. Button numbers are 0-indexed (3 = back / "button 4",
        // 4 = forward / "button 5").
        Action::MouseBack => macos::post_other_button(3),
        Action::MouseForward => macos::post_other_button(4),
        // ── Editing ───────────────────────────────────────────────────────
        Action::Copy => macos::post_key(VK_C, cmd),
        Action::Paste => macos::post_key(VK_V, cmd),
        Action::Cut => macos::post_key(VK_X, cmd),
        Action::Undo => macos::post_key(VK_Z, cmd),
        Action::Redo => macos::post_key(VK_Z, cmd | shift),
        Action::SelectAll => macos::post_key(VK_A, cmd),
        Action::Find => macos::post_key(VK_F, cmd),
        Action::Save => macos::post_key(VK_S, cmd),
        // ── Browser / Navigation ──────────────────────────────────────────
        // BrowserBack/Forward: Cmd+[ / Cmd+] as keyboard fallback; hook
        // layer handles the physical mouse buttons directly.
        // kVK_ANSI_LeftBracket = 0x21, kVK_ANSI_RightBracket = 0x1E
        Action::BrowserBack => macos::post_key(0x21, cmd),
        Action::BrowserForward => macos::post_key(0x1E, cmd),
        Action::NewTab => macos::post_key(VK_T, cmd),
        Action::CloseTab => macos::post_key(VK_W, cmd),
        Action::ReopenTab => macos::post_key(VK_T, cmd | shift),
        Action::NextTab => macos::post_key(VK_TAB, ctrl),
        Action::PrevTab => macos::post_key(VK_TAB, ctrl | shift),
        Action::ReloadPage => macos::post_key(VK_R, cmd),
        // ── Navigation / Window: posted straight to the Dock ──────────────
        // Synthesising these shortcuts is unreliable — the WindowServer
        // matcher needs the exact configured key (incl. the Fn flag) and
        // Show Desktop ignores synthetic events entirely — so they go to the
        // Dock via `CoreDockSendNotification`, which fires regardless of the
        // user's keyboard settings.
        Action::MissionControl => macos::mission_control(),
        Action::AppExpose => macos::app_expose(),
        Action::PreviousDesktop => macos::previous_desktop(),
        Action::NextDesktop => macos::next_desktop(),
        Action::ShowDesktop => macos::show_desktop(),
        Action::LaunchpadShow => macos::launchpad(),
        // ── System ────────────────────────────────────────────────────────
        // Lock screen = Cmd+Ctrl+Q (kVK_ANSI_Q = 0x0C)
        Action::LockScreen => macos::post_key(0x0C, cmd | ctrl),
        // Screenshot = Cmd+Shift+3 (kVK_ANSI_3 = 0x14)
        Action::Screenshot => macos::post_key(0x14, cmd | shift),
        // Capture region to clipboard = Cmd+Shift+Ctrl+4 (kVK_ANSI_4 = 0x15)
        Action::CaptureRegion => macos::post_key(0x15, cmd | shift | ctrl),
        // ── Media ─────────────────────────────────────────────────────────
        // Media/volume controls are NX system-defined keys, not ordinary
        // keyboard virtual-key events. Posting kVK_Volume* through
        // CGEventCreateKeyboardEvent is ignored by macOS' volume handler.
        Action::PlayPause => macos::post_media_key(macos::NX_KEYTYPE_PLAY),
        Action::NextTrack => macos::post_media_key(macos::NX_KEYTYPE_NEXT),
        Action::PrevTrack => macos::post_media_key(macos::NX_KEYTYPE_PREVIOUS),
        Action::VolumeUp => macos::post_media_key(macos::NX_KEYTYPE_SOUND_UP),
        Action::VolumeDown => macos::post_media_key(macos::NX_KEYTYPE_SOUND_DOWN),
        Action::MuteVolume => macos::post_media_key(macos::NX_KEYTYPE_MUTE),
        // ── DPI / SmartShift: handled at hook/HID layer ───────────────────
        Action::CycleDpiPresets | Action::SetDpiPreset(_) | Action::ToggleSmartShift => {
            tracing::debug!(
                action = action.label(),
                "device action handled by hook/HID layer"
            );
        }
        // ── Scroll ────────────────────────────────────────────────────────
        Action::ScrollUp
        | Action::ScrollDown
        | Action::HorizontalScrollLeft
        | Action::HorizontalScrollRight => macos::post_scroll(action),
        // ── Custom ────────────────────────────────────────────────────────
        Action::CustomShortcut(combo) => {
            // P1.3: post the recorded chord. `key_code == 0` is the
            // "modifier-only placeholder" the recorder UI rejects;
            // skip it here too so a malformed config doesn't fire
            // bare modifier presses.
            if combo.key_code == 0 {
                tracing::warn!(
                    chord = %combo.rendered_label(),
                    "CustomShortcut with no key code — press ignored"
                );
                return;
            }
            let mut flags = CGEventFlags::CGEventFlagNull;
            if combo.modifiers & KeyCombo::MOD_CMD != 0 {
                flags |= CGEventFlags::CGEventFlagCommand;
            }
            if combo.modifiers & KeyCombo::MOD_SHIFT != 0 {
                flags |= CGEventFlags::CGEventFlagShift;
            }
            if combo.modifiers & KeyCombo::MOD_CTRL != 0 {
                flags |= CGEventFlags::CGEventFlagControl;
            }
            if combo.modifiers & KeyCombo::MOD_OPTION != 0 {
                flags |= CGEventFlags::CGEventFlagAlternate;
            }
            macos::post_key(combo.key_code, flags);
        }
    }
}

/// Windows implementation: synthesise events via `SendInput`. macOS
/// window-manager actions map to their Windows equivalents; `CustomShortcut`
/// maps macOS `kVK_*` codes to Windows virtual-key codes (Cmd → Ctrl).
#[cfg(target_os = "windows")]
fn execute_windows(action: &Action) {
    match action {
        Action::LeftClick => windows::post_click(windows::MouseButton::Left),
        Action::RightClick => windows::post_click(windows::MouseButton::Right),
        Action::MiddleClick => windows::post_click(windows::MouseButton::Middle),
        Action::MouseBack => windows::post_click(windows::MouseButton::Back),
        Action::MouseForward => windows::post_click(windows::MouseButton::Forward),
        Action::Copy => windows::post_key(windows::VK_C, &[windows::VK_CONTROL]),
        Action::Paste => windows::post_key(windows::VK_V, &[windows::VK_CONTROL]),
        Action::Cut => windows::post_key(windows::VK_X, &[windows::VK_CONTROL]),
        Action::Undo => windows::post_key(windows::VK_Z, &[windows::VK_CONTROL]),
        Action::Redo => windows::post_key(windows::VK_Y, &[windows::VK_CONTROL]),
        Action::SelectAll => windows::post_key(windows::VK_A, &[windows::VK_CONTROL]),
        Action::Find => windows::post_key(windows::VK_F, &[windows::VK_CONTROL]),
        Action::Save => windows::post_key(windows::VK_S, &[windows::VK_CONTROL]),
        Action::BrowserBack => windows::post_key(windows::VK_BROWSER_BACK, &[]),
        Action::BrowserForward => windows::post_key(windows::VK_BROWSER_FORWARD, &[]),
        Action::NewTab => windows::post_key(windows::VK_T, &[windows::VK_CONTROL]),
        Action::CloseTab => windows::post_key(windows::VK_W, &[windows::VK_CONTROL]),
        Action::ReopenTab => {
            windows::post_key(windows::VK_T, &[windows::VK_CONTROL, windows::VK_SHIFT]);
        }
        Action::NextTab => windows::post_key(windows::VK_TAB, &[windows::VK_CONTROL]),
        Action::PrevTab => {
            windows::post_key(windows::VK_TAB, &[windows::VK_CONTROL, windows::VK_SHIFT]);
        }
        Action::ReloadPage => windows::post_key(windows::VK_R, &[windows::VK_CONTROL]),
        Action::MissionControl | Action::AppExpose => {
            windows::post_key(windows::VK_TAB, &[windows::VK_LWIN]);
        }
        Action::PreviousDesktop => {
            windows::post_key(windows::VK_LEFT, &[windows::VK_LWIN, windows::VK_CONTROL]);
        }
        Action::NextDesktop => {
            windows::post_key(windows::VK_RIGHT, &[windows::VK_LWIN, windows::VK_CONTROL]);
        }
        Action::ShowDesktop => windows::post_key(windows::VK_D, &[windows::VK_LWIN]),
        Action::LaunchpadShow => windows::post_key(windows::VK_LWIN, &[]),
        Action::LockScreen => windows::post_key(windows::VK_L, &[windows::VK_LWIN]),
        // Win+Shift+S opens the snip overlay, which serves both full-screen
        // and region capture on Windows.
        Action::Screenshot | Action::CaptureRegion => {
            windows::post_key(windows::VK_S, &[windows::VK_LWIN, windows::VK_SHIFT]);
        }
        Action::PlayPause => windows::post_key(windows::VK_MEDIA_PLAY_PAUSE, &[]),
        Action::NextTrack => windows::post_key(windows::VK_MEDIA_NEXT_TRACK, &[]),
        Action::PrevTrack => windows::post_key(windows::VK_MEDIA_PREV_TRACK, &[]),
        Action::VolumeUp => windows::post_key(windows::VK_VOLUME_UP, &[]),
        Action::VolumeDown => windows::post_key(windows::VK_VOLUME_DOWN, &[]),
        Action::MuteVolume => windows::post_key(windows::VK_VOLUME_MUTE, &[]),
        Action::CycleDpiPresets | Action::SetDpiPreset(_) | Action::ToggleSmartShift => {
            tracing::debug!(
                action = action.label(),
                "device action handled by hook/HID layer"
            );
        }
        Action::ScrollUp
        | Action::ScrollDown
        | Action::HorizontalScrollLeft
        | Action::HorizontalScrollRight => windows::post_scroll(action),
        Action::CustomShortcut(combo) => windows::post_custom_shortcut(combo),
        Action::None => {}
    }
}

/// Synthesise a horizontal scroll of `delta` wheel lines at the current focus.
///
/// Used by the gesture/thumbwheel capture watcher to re-inject the MX thumb
/// wheel's scrolling after the wheel has been diverted over HID++ to capture its
/// click. `delta` is the device's raw rotation; its sign follows the wheel's
/// rotation convention and its magnitude (one line per rotation increment) may
/// need tuning per device, since the diverted resolution differs from native.
///
/// No-op (logs nothing) on platforms without a supported injection mechanism.
pub fn post_horizontal_scroll(delta: i32) {
    #[cfg(target_os = "macos")]
    macos::post_horizontal_scroll(delta);

    // `delta` is already in "one line per rotation increment" units (see doc
    // above), which matches REL_HWHEEL's convention of one unit per detent.
    // This is intentionally different from Action::HorizontalScrollLeft/Right,
    // which hardcode ±3 as a fixed "scroll tick" with no device delta involved.
    #[cfg(target_os = "linux")]
    linux::scroll(evdev::RelativeAxisCode::REL_HWHEEL, delta);

    #[cfg(target_os = "windows")]
    windows::post_horizontal_scroll(delta);

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    let _ = delta;
}

/// Return the `/dev/input/eventN` node for the action-injector uinput device,
/// initialising it if needed.
///
/// Intended for debugging and manual smoke-testing (e.g. attaching `evtest`
/// before firing [`execute`]). Returns `None` on non-Linux platforms or
/// when the device could not be created (e.g. `/dev/uinput` not writable).
#[cfg(target_os = "linux")]
#[must_use]
pub fn action_device_path() -> Option<std::path::PathBuf> {
    linux::device_node()
}

// ── macOS virtual key codes ────────────────────────────────────────────────
// Source: <HIToolbox/Events.h> kVK_* constants. Values are layout-independent
// for the US ANSI keyboard.
#[cfg(target_os = "macos")]
const VK_A: u16 = 0x00;
#[cfg(target_os = "macos")]
const VK_C: u16 = 0x08;
#[cfg(target_os = "macos")]
const VK_F: u16 = 0x03;
#[cfg(target_os = "macos")]
const VK_R: u16 = 0x0F;
#[cfg(target_os = "macos")]
const VK_S: u16 = 0x01;
#[cfg(target_os = "macos")]
const VK_T: u16 = 0x11;
#[cfg(target_os = "macos")]
const VK_V: u16 = 0x09;
#[cfg(target_os = "macos")]
const VK_W: u16 = 0x0D;
#[cfg(target_os = "macos")]
const VK_X: u16 = 0x07;
#[cfg(target_os = "macos")]
const VK_Z: u16 = 0x06;
#[cfg(target_os = "macos")]
const VK_TAB: u16 = 0x30;

/// Stamped into the `EVENT_SOURCE_USER_DATA` field of every mouse event
/// [`execute`] synthesizes on macOS, so OpenLogi's own `CGEventTap` can
/// recognize and skip its own injections. Without it, a gesture/button action
/// that posts a mouse button (e.g. a remapped `MiddleClick`) would re-enter the
/// hook — and for a gesture button, be misread as a fresh hold, looping. The
/// value is arbitrary but distinctive ("OLGI"); real events carry `0` here.
pub const SYNTHETIC_EVENT_USER_DATA: i64 = 0x4F4C_4749;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "linux")]
mod linux;

/// Translate a macOS virtual key code (`kVK_*`, captured when a `CustomShortcut`
/// was recorded on macOS) to the equivalent Windows virtual-key code, so a chord
/// synced from a Mac fires the right key on Windows.
///
/// Covers letters, digits, the ANSI punctuation keys, whitespace/editing keys,
/// navigation, and F1–F20 — every key a shortcut realistically uses. Modifier
/// keys are applied separately from `KeyCombo::modifiers`; the numeric keypad,
/// media, and volume keys are intentionally omitted (they are modifiers or
/// already have dedicated actions). `None` for an unmapped code, which
/// `post_custom_shortcut` warns-and-drops.
///
/// Source codes: `<HIToolbox/Events.h>` kVK_* constants. Targets: Win32
/// virtual-key codes (letters/digits are their ASCII values; F1 = 0x70).
#[cfg_attr(
    not(target_os = "windows"),
    allow(
        dead_code,
        reason = "pure key-code table is exercised by host unit tests; its only runtime caller is the Windows-gated post_custom_shortcut"
    )
)]
fn mac_virtual_key_to_windows(key_code: u16) -> Option<u16> {
    Some(match key_code {
        // ── Letters (Windows VK_A..VK_Z = ASCII 'A'..'Z') ──
        0x00 => 0x41, // A
        0x0B => 0x42, // B
        0x08 => 0x43, // C
        0x02 => 0x44, // D
        0x0E => 0x45, // E
        0x03 => 0x46, // F
        0x05 => 0x47, // G
        0x04 => 0x48, // H
        0x22 => 0x49, // I
        0x26 => 0x4A, // J
        0x28 => 0x4B, // K
        0x25 => 0x4C, // L
        0x2E => 0x4D, // M
        0x2D => 0x4E, // N
        0x1F => 0x4F, // O
        0x23 => 0x50, // P
        0x0C => 0x51, // Q
        0x0F => 0x52, // R
        0x01 => 0x53, // S
        0x11 => 0x54, // T
        0x20 => 0x55, // U
        0x09 => 0x56, // V
        0x0D => 0x57, // W
        0x07 => 0x58, // X
        0x10 => 0x59, // Y
        0x06 => 0x5A, // Z
        // ── Digits (Windows VK_0..VK_9 = ASCII '0'..'9') ──
        0x1D => 0x30, // 0
        0x12 => 0x31, // 1
        0x13 => 0x32, // 2
        0x14 => 0x33, // 3
        0x15 => 0x34, // 4
        0x17 => 0x35, // 5
        0x16 => 0x36, // 6
        0x1A => 0x37, // 7
        0x1C => 0x38, // 8
        0x19 => 0x39, // 9
        // ── ANSI punctuation (Windows VK_OEM_*) ──
        0x1B => 0xBD, // -  VK_OEM_MINUS
        0x18 => 0xBB, // =  VK_OEM_PLUS
        0x21 => 0xDB, // [  VK_OEM_4
        0x1E => 0xDD, // ]  VK_OEM_6
        0x2A => 0xDC, // \  VK_OEM_5
        0x29 => 0xBA, // ;  VK_OEM_1
        0x27 => 0xDE, // '  VK_OEM_7
        0x2B => 0xBC, // ,  VK_OEM_COMMA
        0x2F => 0xBE, // .  VK_OEM_PERIOD
        0x2C => 0xBF, // /  VK_OEM_2
        0x32 => 0xC0, // `  VK_OEM_3
        // ── Whitespace / editing ──
        0x24 => 0x0D, // Return     VK_RETURN
        0x30 => 0x09, // Tab        VK_TAB
        0x31 => 0x20, // Space      VK_SPACE
        0x33 => 0x08, // Backspace  VK_BACK
        0x35 => 0x1B, // Escape     VK_ESCAPE
        // ── Navigation ──
        0x73 => 0x24, // Home          VK_HOME
        0x77 => 0x23, // End           VK_END
        0x74 => 0x21, // PageUp        VK_PRIOR
        0x79 => 0x22, // PageDown      VK_NEXT
        0x75 => 0x2E, // ForwardDelete VK_DELETE
        0x7B => 0x25, // LeftArrow     VK_LEFT
        0x7C => 0x27, // RightArrow    VK_RIGHT
        0x7D => 0x28, // DownArrow     VK_DOWN
        0x7E => 0x26, // UpArrow       VK_UP
        // ── Function keys (Windows VK_F1 = 0x70, sequential through VK_F24) ──
        0x7A => 0x70, // F1
        0x78 => 0x71, // F2
        0x63 => 0x72, // F3
        0x76 => 0x73, // F4
        0x60 => 0x74, // F5
        0x61 => 0x75, // F6
        0x62 => 0x76, // F7
        0x64 => 0x77, // F8
        0x65 => 0x78, // F9
        0x6D => 0x79, // F10
        0x67 => 0x7A, // F11
        0x6F => 0x7B, // F12
        0x69 => 0x7C, // F13
        0x6B => 0x7D, // F14
        0x71 => 0x7E, // F15
        0x6A => 0x7F, // F16
        0x40 => 0x80, // F17
        0x4F => 0x81, // F18
        0x50 => 0x82, // F19
        0x5A => 0x83, // F20
        _ => return None,
    })
}

#[cfg(target_os = "windows")]
mod windows;

#[cfg(test)]
#[allow(clippy::expect_used, reason = "expect/unwrap are idiomatic in tests")]
mod tests {
    #[test]
    fn custom_shortcut_keycodes_map_across_categories() {
        use super::mac_virtual_key_to_windows;
        // One representative per category, checked against independently-known
        // (kVK → Win32 VK) facts, so a systematic error (swapped digits,
        // off-by-one F-keys, a wrong OEM code) is caught without restating the
        // whole table.
        assert_eq!(mac_virtual_key_to_windows(0x00), Some(0x41)); // A → VK_A
        assert_eq!(mac_virtual_key_to_windows(0x12), Some(0x31)); // 1 → VK_1
        assert_eq!(mac_virtual_key_to_windows(0x7A), Some(0x70)); // F1 → VK_F1
        assert_eq!(mac_virtual_key_to_windows(0x7B), Some(0x25)); // LeftArrow → VK_LEFT
        assert_eq!(mac_virtual_key_to_windows(0x31), Some(0x20)); // Space → VK_SPACE
        assert_eq!(mac_virtual_key_to_windows(0x29), Some(0xBA)); // ; → VK_OEM_1
        assert_eq!(mac_virtual_key_to_windows(0x37), None); // Command is a modifier, not a key
    }

    // ── modifiers_to_keycodes ─────────────────────────────────────────────────

    #[cfg(target_os = "linux")]
    mod modifier_mapping {
        use evdev::KeyCode;

        use crate::inject::linux::modifiers_to_keycodes;
        use openlogi_core::binding::KeyCombo;

        #[test]
        fn mod_cmd_alone_maps_to_ctrl() {
            assert_eq!(
                modifiers_to_keycodes(KeyCombo::MOD_CMD),
                vec![KeyCode::KEY_LEFTCTRL]
            );
        }

        #[test]
        fn mod_ctrl_alone_maps_to_ctrl() {
            assert_eq!(
                modifiers_to_keycodes(KeyCombo::MOD_CTRL),
                vec![KeyCode::KEY_LEFTCTRL]
            );
        }

        #[test]
        fn mod_cmd_and_ctrl_together_produce_single_ctrl() {
            // Both bits set must not push KEY_LEFTCTRL twice.
            assert_eq!(
                modifiers_to_keycodes(KeyCombo::MOD_CMD | KeyCombo::MOD_CTRL),
                vec![KeyCode::KEY_LEFTCTRL]
            );
        }

        #[test]
        fn all_modifiers_produce_canonical_order() {
            let mods = modifiers_to_keycodes(
                KeyCombo::MOD_CMD | KeyCombo::MOD_SHIFT | KeyCombo::MOD_OPTION,
            );
            assert_eq!(
                mods,
                vec![
                    KeyCode::KEY_LEFTCTRL,
                    KeyCode::KEY_LEFTSHIFT,
                    KeyCode::KEY_LEFTALT
                ]
            );
        }

        #[test]
        fn no_modifiers_produces_empty_vec() {
            assert!(modifiers_to_keycodes(0).is_empty());
        }
    }

    // ── macos_vk_to_linux ────────────────────────────────────────────────────

    #[cfg(target_os = "linux")]
    mod vk_mapping {
        use evdev::KeyCode;

        use crate::inject::linux::macos_vk_to_linux;

        #[test]
        fn common_letters_map_correctly() {
            assert_eq!(macos_vk_to_linux(0x08), Some(KeyCode::KEY_C)); // kVK_ANSI_C
            assert_eq!(macos_vk_to_linux(0x09), Some(KeyCode::KEY_V)); // kVK_ANSI_V
            assert_eq!(macos_vk_to_linux(0x07), Some(KeyCode::KEY_X)); // kVK_ANSI_X
            assert_eq!(macos_vk_to_linux(0x00), Some(KeyCode::KEY_A)); // kVK_ANSI_A
            assert_eq!(macos_vk_to_linux(0x06), Some(KeyCode::KEY_Z)); // kVK_ANSI_Z
            assert_eq!(macos_vk_to_linux(0x0D), Some(KeyCode::KEY_W)); // kVK_ANSI_W
        }

        #[test]
        fn digits_map_correctly() {
            assert_eq!(macos_vk_to_linux(0x12), Some(KeyCode::KEY_1)); // kVK_ANSI_1
            assert_eq!(macos_vk_to_linux(0x1D), Some(KeyCode::KEY_0)); // kVK_ANSI_0
        }

        #[test]
        fn arrow_keys_map_correctly() {
            assert_eq!(macos_vk_to_linux(0x7B), Some(KeyCode::KEY_LEFT));
            assert_eq!(macos_vk_to_linux(0x7C), Some(KeyCode::KEY_RIGHT));
            assert_eq!(macos_vk_to_linux(0x7D), Some(KeyCode::KEY_DOWN));
            assert_eq!(macos_vk_to_linux(0x7E), Some(KeyCode::KEY_UP));
        }

        #[test]
        fn function_keys_map_correctly() {
            assert_eq!(macos_vk_to_linux(0x7A), Some(KeyCode::KEY_F1)); // kVK_F1
            assert_eq!(macos_vk_to_linux(0x78), Some(KeyCode::KEY_F2)); // kVK_F2
            assert_eq!(macos_vk_to_linux(0x76), Some(KeyCode::KEY_F4)); // kVK_F4
            assert_eq!(macos_vk_to_linux(0x60), Some(KeyCode::KEY_F5)); // kVK_F5
            assert_eq!(macos_vk_to_linux(0x6F), Some(KeyCode::KEY_F12)); // kVK_F12
        }

        #[test]
        fn nav_keys_map_correctly() {
            assert_eq!(macos_vk_to_linux(0x73), Some(KeyCode::KEY_HOME));
            assert_eq!(macos_vk_to_linux(0x77), Some(KeyCode::KEY_END));
            assert_eq!(macos_vk_to_linux(0x74), Some(KeyCode::KEY_PAGEUP));
            assert_eq!(macos_vk_to_linux(0x79), Some(KeyCode::KEY_PAGEDOWN));
            assert_eq!(macos_vk_to_linux(0x75), Some(KeyCode::KEY_DELETE));
        }

        #[test]
        fn brackets_follow_ansi_layout() {
            // kVK_ANSI_LeftBracket=0x21 → KEY_LEFTBRACE, RightBracket=0x1E → KEY_RIGHTBRACE
            assert_eq!(macos_vk_to_linux(0x21), Some(KeyCode::KEY_LEFTBRACE));
            assert_eq!(macos_vk_to_linux(0x1E), Some(KeyCode::KEY_RIGHTBRACE));
        }

        #[test]
        fn unmapped_code_returns_none() {
            assert_eq!(macos_vk_to_linux(0xFF), None);
            assert_eq!(macos_vk_to_linux(0x34), None); // gap in the kVK table
        }
    }
}
