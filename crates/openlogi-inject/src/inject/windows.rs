#![allow(unsafe_code, reason = "SendInput is the Win32 API for synthetic input")]

use std::mem::size_of;

use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYEVENTF_KEYUP, MOUSEEVENTF_HWHEEL,
    MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP,
    MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_WHEEL, MOUSEEVENTF_XDOWN,
    MOUSEEVENTF_XUP, MOUSEINPUT, SendInput,
};

use openlogi_core::binding::{Action, KeyCombo};

const WHEEL_DELTA: i32 = 120;

pub(super) const VK_A: u16 = 0x41;
pub(super) const VK_C: u16 = 0x43;
pub(super) const VK_D: u16 = 0x44;
pub(super) const VK_F: u16 = 0x46;
pub(super) const VK_L: u16 = 0x4C;
pub(super) const VK_R: u16 = 0x52;
pub(super) const VK_S: u16 = 0x53;
pub(super) const VK_T: u16 = 0x54;
pub(super) const VK_V: u16 = 0x56;
pub(super) const VK_W: u16 = 0x57;
pub(super) const VK_X: u16 = 0x58;
pub(super) const VK_Y: u16 = 0x59;
pub(super) const VK_Z: u16 = 0x5A;
pub(super) const VK_TAB: u16 = 0x09;
pub(super) const VK_LEFT: u16 = 0x25;
pub(super) const VK_RIGHT: u16 = 0x27;
pub(super) const VK_SHIFT: u16 = 0x10;
pub(super) const VK_CONTROL: u16 = 0x11;
pub(super) const VK_MENU: u16 = 0x12;
pub(super) const VK_LWIN: u16 = 0x5B;
pub(super) const VK_BROWSER_BACK: u16 = 0xA6;
pub(super) const VK_BROWSER_FORWARD: u16 = 0xA7;
pub(super) const VK_VOLUME_MUTE: u16 = 0xAD;
pub(super) const VK_VOLUME_DOWN: u16 = 0xAE;
pub(super) const VK_VOLUME_UP: u16 = 0xAF;
pub(super) const VK_MEDIA_NEXT_TRACK: u16 = 0xB0;
pub(super) const VK_MEDIA_PREV_TRACK: u16 = 0xB1;
pub(super) const VK_MEDIA_PLAY_PAUSE: u16 = 0xB3;

#[derive(Clone, Copy)]
pub(super) enum MouseButton {
    Left,
    Right,
    Middle,
    /// Extra button 4 ("back").
    Back,
    /// Extra button 5 ("forward").
    Forward,
}

// XBUTTON1/XBUTTON2 from WinUser.h — windows-sys puts them behind the
// Win32_UI_WindowsAndMessaging feature; not worth enabling for two
// integers (same treatment as the VK_* codes above).
const XBUTTON1: i32 = 1;
const XBUTTON2: i32 = 2;

pub(super) fn post_click(button: MouseButton) {
    let (down, up, data) = match button {
        MouseButton::Left => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, 0),
        MouseButton::Right => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, 0),
        MouseButton::Middle => (MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, 0),
        // Extra buttons share the X flag pair; mouseData carries which one.
        MouseButton::Back => (MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP, XBUTTON1),
        MouseButton::Forward => (MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP, XBUTTON2),
    };
    send_inputs(&[mouse_input(down, data), mouse_input(up, data)]);
}

pub(super) fn post_key(vk: u16, modifiers: &[u16]) {
    let mut inputs = Vec::with_capacity(modifiers.len() * 2 + 2);
    for modifier in modifiers {
        inputs.push(key_input(*modifier, false));
    }
    inputs.push(key_input(vk, false));
    inputs.push(key_input(vk, true));
    for modifier in modifiers.iter().rev() {
        inputs.push(key_input(*modifier, true));
    }
    send_inputs(&inputs);
}

pub(super) fn post_scroll(action: &Action) {
    let (flags, data) = match action {
        Action::ScrollUp => (MOUSEEVENTF_WHEEL, WHEEL_DELTA),
        Action::ScrollDown => (MOUSEEVENTF_WHEEL, -WHEEL_DELTA),
        Action::HorizontalScrollLeft => (MOUSEEVENTF_HWHEEL, -WHEEL_DELTA),
        Action::HorizontalScrollRight => (MOUSEEVENTF_HWHEEL, WHEEL_DELTA),
        _ => return,
    };
    send_inputs(&[mouse_input(flags, data)]);
}

pub(super) fn post_horizontal_scroll(delta: i32) {
    if delta == 0 {
        return;
    }
    send_inputs(&[mouse_input(
        MOUSEEVENTF_HWHEEL,
        delta.saturating_mul(WHEEL_DELTA),
    )]);
}

pub(super) fn post_custom_shortcut(combo: &KeyCombo) {
    if combo.key_code == 0 {
        tracing::warn!(
            chord = %combo.rendered_label(),
            "CustomShortcut with no key code; press ignored"
        );
        return;
    }
    let Some(vk) = super::mac_virtual_key_to_windows(combo.key_code) else {
        tracing::warn!(
            key_code = combo.key_code,
            chord = %combo.rendered_label(),
            "CustomShortcut key has no Windows mapping yet; press ignored"
        );
        return;
    };

    let mut modifiers = Vec::new();
    if combo.modifiers & KeyCombo::MOD_CMD != 0 {
        modifiers.push(VK_CONTROL);
    }
    if combo.modifiers & KeyCombo::MOD_SHIFT != 0 {
        modifiers.push(VK_SHIFT);
    }
    if combo.modifiers & KeyCombo::MOD_CTRL != 0 && !modifiers.contains(&VK_CONTROL) {
        modifiers.push(VK_CONTROL);
    }
    if combo.modifiers & KeyCombo::MOD_OPTION != 0 {
        modifiers.push(VK_MENU);
    }
    post_key(vk, &modifiers);
}

fn send_inputs(inputs: &[INPUT]) {
    let Ok(input_count) = u32::try_from(inputs.len()) else {
        tracing::warn!(
            requested = inputs.len(),
            "too many SendInput events requested"
        );
        return;
    };
    let Ok(input_size) = i32::try_from(size_of::<INPUT>()) else {
        tracing::warn!("INPUT size does not fit the Win32 SendInput contract");
        return;
    };
    // SAFETY: inputs.as_ptr()/input_count describe a valid initialized INPUT slice; SendInput copies it and returns the count injected.
    let sent = unsafe { SendInput(input_count, inputs.as_ptr(), input_size) };
    if sent != input_count {
        tracing::warn!(
            requested = inputs.len(),
            sent,
            "SendInput accepted fewer events than requested"
        );
    }
}

fn key_input(vk: u16, key_up: bool) -> INPUT {
    let mut flags = 0;
    if key_up {
        flags |= KEYEVENTF_KEYUP;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn mouse_input(flags: u32, data: i32) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: u32::from_ne_bytes(data.to_ne_bytes()),
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}
