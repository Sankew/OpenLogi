//! Shared card, badge, label, and loading-frame primitives used by the Home
//! and detail screens and the root connection frames.

use gpui::{
    AnyElement, Div, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement as _, Styled, div, prelude::FluentBuilder as _, px, rgb,
};
use gpui_component::{
    Icon, IconName, Sizable as _, h_flex, spinner::Spinner, tooltip::Tooltip, v_flex,
};
use openlogi_core::device::{DeviceKind, DeviceTransports};
use openlogi_hid::DeviceRoute;

use crate::theme::{self, Palette};

/// Trailing "+" button that opens the pairing window. Present in both screen
/// headers; the empty state carries its own primary "Add Device" CTA, so this
/// never floats alone in an empty header.
pub(super) fn add_device_button(pal: Palette) -> impl IntoElement {
    h_flex()
        .id("header-add-device")
        .flex_shrink_0()
        .size(px(36.))
        .items_center()
        .justify_center()
        .rounded_md()
        .border_1()
        .border_color(pal.border)
        .bg(pal.surface)
        .text_color(pal.text_muted)
        .cursor_pointer()
        .hover(|s| s.bg(pal.surface_hover).text_color(pal.text_primary))
        .tooltip(|window, cx| Tooltip::new(tr!("Add Device")).build(window, cx))
        .child(Icon::new(IconName::Plus).size_4())
        .on_click(|_, _, cx| crate::windows::add_device::open(cx))
}

pub(super) fn panel_card(
    title: SharedString,
    icon: IconName,
    pal: Palette,
    content: AnyElement,
) -> impl IntoElement {
    panel_card_inner(title, icon, pal, content, false)
}

pub(super) fn panel_card_fill(
    title: SharedString,
    icon: IconName,
    pal: Palette,
    content: AnyElement,
) -> impl IntoElement {
    panel_card_inner(title, icon, pal, content, true)
}

fn panel_card_inner(
    title: SharedString,
    icon: IconName,
    pal: Palette,
    content: AnyElement,
    fill_height: bool,
) -> impl IntoElement {
    div()
        .w_full()
        .when(fill_height, gpui::Styled::h_full)
        .max_w_full()
        .min_w_0()
        .rounded_lg()
        .border_1()
        .border_color(pal.border)
        .bg(pal.surface)
        .p_4()
        .child(
            v_flex()
                .gap_3()
                .when(!title.is_empty(), |this| {
                    this.child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .text_color(pal.text_primary)
                            .child(Icon::new(icon).size_4().text_color(pal.text_muted))
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(title),
                            ),
                    )
                })
                .child(content),
        )
}

pub(super) fn status_badge(online: bool, pal: Palette) -> impl IntoElement {
    let (label, color) = if online {
        (tr!("Connected"), theme::STATUS_CONNECTED)
    } else {
        (tr!("Offline"), theme::STATUS_OFFLINE)
    };
    h_flex()
        .gap_1()
        .items_center()
        .rounded_full()
        .border_1()
        .border_color(pal.border)
        .px_2()
        .py_1()
        .text_xs()
        .text_color(pal.text_muted)
        .child(div().size_1p5().rounded_full().bg(rgb(color)))
        .child(label)
}

pub(super) fn route_label(route: Option<&DeviceRoute>) -> String {
    match route {
        Some(DeviceRoute::Bolt { .. }) => tr!("Bolt receiver").to_string(),
        Some(DeviceRoute::Unifying { .. }) => tr!("Unifying receiver").to_string(),
        Some(DeviceRoute::Direct { .. }) => tr!("Direct connection").to_string(),
        None => tr!("Unavailable").to_string(),
    }
}

/// Connection-type glyph for a gallery card: a dongle for receiver-paired
/// devices, a USB mark for radio-less direct ones (a wired keyboard is only
/// ever on the cable), a Bluetooth mark for the rest.
///
/// The route says how the device is *addressed*, not what medium carries it,
/// so `Direct` alone can't pick a glyph — the firmware transport table
/// (HID++ 0x0003) disambiguates. A radio-capable device on a direct route
/// keeps the Bluetooth mark: it *may* be on a cable right now, but the
/// current link medium isn't reported, and Bluetooth is how such devices are
/// normally attached.
pub(super) fn connection_icon_path(
    route: Option<&DeviceRoute>,
    transports: Option<&DeviceTransports>,
) -> &'static str {
    match route {
        Some(DeviceRoute::Bolt { .. }) => "action-icons/bolt.svg",
        Some(DeviceRoute::Unifying { .. }) => "action-icons/unifying.svg",
        // Explicit arms (not `_`) so a new DeviceRoute variant trips the
        // compiler here, matching the exhaustive sibling `route_label`.
        Some(DeviceRoute::Direct { .. }) | None => match transports {
            // No Bluetooth radio at all ⇒ the direct link can only be the
            // cable. eQuad counts as wired-capable here: eQuad is
            // receiver-only by definition, so it is never the *direct* link —
            // an equad-only table still means this connection is a cable.
            Some(t) if (t.usb || t.equad) && !t.bluetooth && !t.btle => "action-icons/usb.svg",
            // Unknown transports (no 0x0003 snapshot, or an all-false table)
            // keep the old default.
            _ => "action-icons/bluetooth.svg",
        },
    }
}

pub(super) fn kind_label(kind: DeviceKind) -> String {
    match kind {
        DeviceKind::Mouse => tr!("Mouse").to_string(),
        DeviceKind::Keyboard => tr!("Keyboard").to_string(),
        DeviceKind::Numpad => tr!("Numpad").to_string(),
        DeviceKind::Presenter => tr!("Presenter").to_string(),
        DeviceKind::Remote => tr!("Remote").to_string(),
        DeviceKind::Trackball => tr!("Trackball").to_string(),
        DeviceKind::Touchpad => tr!("Touchpad").to_string(),
        DeviceKind::Tablet => tr!("Tablet").to_string(),
        DeviceKind::Gamepad => tr!("Gamepad").to_string(),
        DeviceKind::Joystick => tr!("Joystick").to_string(),
        DeviceKind::Headset => tr!("Headset").to_string(),
        DeviceKind::Unknown => tr!("Device").to_string(),
    }
}

pub(super) fn battery_color(percentage: u8) -> u32 {
    match percentage {
        0..=20 => 0x00ef_4444,
        21..=50 => theme::STATUS_CONNECTING,
        _ => theme::STATUS_CONNECTED,
    }
}

/// Centered spinner over a muted one-line caption — the quiet "still working"
/// body shared by the pre-connection frame and the scanning state, so the two
/// loading phases render as one continuous frame with only the caption
/// changing. The spinner's repeating animation re-renders the window every
/// frame while mounted, which is fine *because* both loading states are
/// bounded: the connecting frame downgrades to the static
/// [`unreachable_body`](super::unreachable_body) when no snapshot arrives, and
/// the scanning state ends with the agent reporting `Ready` or `Unavailable`.
pub(super) fn loading_body(caption: SharedString, pal: Palette) -> Div {
    v_flex()
        .items_center()
        .justify_center()
        .gap_3()
        .child(Spinner::new().large().color(pal.text_muted))
        .child(div().text_sm().text_color(pal.text_muted).child(caption))
}

/// Static centered notice — icon, headline, muted caption — for the
/// connection-problem frames. Unlike [`loading_body`] there is deliberately
/// no animation: these frames can stay up indefinitely, and an infinite
/// animation would pin the render loop for as long as they do (the same
/// reasoning as the status dot's fixed glow).
pub(super) fn notice_body(headline: SharedString, caption: SharedString, pal: Palette) -> Div {
    v_flex()
        .items_center()
        .justify_center()
        .gap_4()
        .p_8()
        .child(
            Icon::new(IconName::TriangleAlert)
                .size_8()
                .text_color(rgb(theme::STATUS_CONNECTING)),
        )
        .child(
            div()
                .text_xl()
                .font_weight(FontWeight::SEMIBOLD)
                .child(headline),
        )
        .child(
            div()
                .max_w(px(440.))
                .text_sm()
                .text_center()
                .text_color(pal.text_muted)
                .child(caption),
        )
}

#[cfg(test)]
mod tests {
    use super::{DeviceRoute, DeviceTransports, connection_icon_path};

    #[test]
    fn connection_icon_matches_route() {
        let bolt = DeviceRoute::Bolt {
            receiver_uid: "r".into(),
            slot: 1,
        };
        let uni = DeviceRoute::Unifying {
            receiver_uid: "r".into(),
            slot: 1,
        };
        let direct = DeviceRoute::Direct {
            vendor_id: 0x046d,
            product_id: 0xb019,
        };
        // Firmware transport tables (HID++ 0x0003): a wired-only device (G513),
        // a Bluetooth-capable one (MX Master on a cable or BT), and BLE-direct.
        let wired = DeviceTransports {
            usb: true,
            ..DeviceTransports::default()
        };
        let bt = DeviceTransports {
            usb: true,
            bluetooth: true,
            ..DeviceTransports::default()
        };
        let btle = DeviceTransports {
            btle: true,
            ..DeviceTransports::default()
        };
        assert_eq!(
            connection_icon_path(Some(&bolt), None),
            "action-icons/bolt.svg"
        );
        assert_eq!(
            connection_icon_path(Some(&uni), None),
            "action-icons/unifying.svg"
        );
        // Direct + radio-less firmware = the cable is the only possible link.
        assert_eq!(
            connection_icon_path(Some(&direct), Some(&wired)),
            "action-icons/usb.svg"
        );
        // eQuad is receiver-only, so an equad-only table on a *direct* route
        // still means a cable — not Bluetooth.
        let equad_only = DeviceTransports {
            equad: true,
            ..DeviceTransports::default()
        };
        assert_eq!(
            connection_icon_path(Some(&direct), Some(&equad_only)),
            "action-icons/usb.svg"
        );
        // An all-false table is "unknown", not "wired".
        assert_eq!(
            connection_icon_path(Some(&direct), Some(&DeviceTransports::default())),
            "action-icons/bluetooth.svg"
        );
        // Direct + any radio keeps the Bluetooth mark.
        assert_eq!(
            connection_icon_path(Some(&direct), Some(&bt)),
            "action-icons/bluetooth.svg"
        );
        assert_eq!(
            connection_icon_path(Some(&direct), Some(&btle)),
            "action-icons/bluetooth.svg"
        );
        // Unknown transports (no 0x0003 snapshot) keep the old default.
        assert_eq!(
            connection_icon_path(Some(&direct), None),
            "action-icons/bluetooth.svg"
        );
        // No route (e.g. a synthetic/placeholder card) falls back to Bluetooth.
        assert_eq!(
            connection_icon_path(None, None),
            "action-icons/bluetooth.svg"
        );
    }
}
