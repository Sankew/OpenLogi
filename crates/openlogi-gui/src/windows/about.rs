//! The About window — a small standalone OS window (menu / footer link)
//! showing the wordmark, version, a one-line description, and outbound links.
//!
//! The app icon is intentionally omitted: `img()` resolves filesystem paths,
//! and `design/icon/openlogi.png` isn't embedded as a runtime asset, so the
//! path wouldn't resolve inside a packaged `.app`. A text wordmark is correct
//! everywhere; embedding the icon (`include_bytes!` + an `AssetSource`) remains
//! a follow-up.

use gpui::{
    App, Context, FontWeight, IntoElement, ParentElement as _, Render, Size, Styled as _,
    Subscription, Window, div, px,
};
use gpui_component::{button::Button, h_flex, v_flex};

use crate::theme;
use crate::windows::{self, AuxWindow};

const REPO_URL: &str = "https://github.com/AprilNEA/OpenLogi";
const RELEASES_URL: &str = "https://github.com/AprilNEA/OpenLogi/releases/latest";

/// Standalone About window root view.
pub struct AboutView {
    #[allow(dead_code, reason = "held to keep the appearance observer alive")]
    appearance_obs: Option<Subscription>,
}

impl AboutView {
    fn new(_: &mut Context<Self>) -> Self {
        Self {
            appearance_obs: None,
        }
    }
}

impl AuxWindow for AboutView {
    fn set_appearance_obs(&mut self, sub: Subscription) {
        self.appearance_obs = Some(sub);
    }
}

/// Open the About window, or focus it if it's already open.
pub fn open(cx: &mut App) {
    windows::open_or_focus(
        |reg| &mut reg.about,
        "About OpenLogi",
        Size::new(px(360.), px(420.)),
        AboutView::new,
        cx,
    );
}

impl Render for AboutView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = theme::palette(cx);

        v_flex()
            .size_full()
            .bg(pal.bg)
            .text_color(pal.text_primary)
            .items_center()
            .justify_center()
            .gap_3()
            .p_8()
            .child(
                div()
                    .text_2xl()
                    .font_weight(FontWeight::BOLD)
                    .child("OpenLogi"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(pal.text_muted)
                    .child(concat!("v", env!("CARGO_PKG_VERSION"))),
            )
            .child(
                div()
                    .max_w(px(280.))
                    .text_sm()
                    .text_center()
                    .text_color(pal.text_muted)
                    .child("开源的 Logitech 鼠标配置工具 —— DPI、SmartShift、按键绑定与手势。"),
            )
            .child(
                h_flex()
                    .gap_3()
                    .pt_2()
                    .child(
                        Button::new("about-repo")
                            .outline()
                            .label("GitHub")
                            .on_click(|_, _, cx| cx.open_url(REPO_URL)),
                    )
                    .child(
                        Button::new("about-releases")
                            .outline()
                            .label("Releases")
                            .on_click(|_, _, cx| cx.open_url(RELEASES_URL)),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(pal.text_muted)
                    .child("Licensed under MIT OR Apache-2.0"),
            )
    }
}
