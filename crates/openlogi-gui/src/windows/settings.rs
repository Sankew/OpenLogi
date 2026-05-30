//! The Settings window — a standalone OS window (⌘, / menu / footer link)
//! exposing the app-wide preferences in [`openlogi_core::config::AppSettings`].
//!
//! Two toggles for now, so the layout is a hand-rolled form rather than
//! gpui-component's [`Settings`](gpui_component::setting::Settings) widget
//! (whose 250px page sidebar would dwarf two switches). When the preference
//! set grows enough to warrant pages, this can migrate to that widget.

use gpui::{
    App, BorrowAppContext as _, Context, FontWeight, IntoElement, ParentElement as _, Render, Size,
    Styled as _, Subscription, Window, div, px,
};
use gpui_component::{group_box::GroupBox, h_flex, switch::Switch, v_flex};

use crate::state::AppState;
use crate::theme::{self, Palette};
use crate::windows::{self, AuxWindow};

/// Standalone Settings window root view.
pub struct SettingsView {
    #[allow(dead_code, reason = "held to keep the appearance observer alive")]
    appearance_obs: Option<Subscription>,
}

impl SettingsView {
    fn new(_: &mut Context<Self>) -> Self {
        Self {
            appearance_obs: None,
        }
    }
}

impl AuxWindow for SettingsView {
    fn set_appearance_obs(&mut self, sub: Subscription) {
        self.appearance_obs = Some(sub);
    }
}

/// Open the Settings window, or focus it if it's already open.
pub fn open(cx: &mut App) {
    windows::open_or_focus(
        |reg| &mut reg.settings,
        "Settings",
        Size::new(px(520.), px(420.)),
        SettingsView::new,
        cx,
    );
}

impl Render for SettingsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = theme::palette(cx);
        let (launch, updates) = cx.try_global::<AppState>().map_or((false, false), |s| {
            let a = s.app_settings();
            (a.launch_at_login, a.check_for_updates)
        });

        v_flex()
            .size_full()
            .bg(pal.bg)
            .text_color(pal.text_primary)
            .p_6()
            .gap_6()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Settings"),
            )
            .child(
                GroupBox::new()
                    .title("通用")
                    .child(setting_row(
                        Switch::new("launch-at-login")
                            .checked(launch)
                            .on_click(cx.listener(|_, checked: &bool, _, cx| {
                                let enabled = *checked;
                                cx.update_global::<AppState, _>(move |s, _| {
                                    s.set_launch_at_login(enabled);
                                });
                                cx.notify();
                            })),
                        "开机自启",
                        "登录 macOS 时自动启动 OpenLogi。",
                        pal,
                    ))
                    .child(setting_row(
                        Switch::new("check-for-updates")
                            .checked(updates)
                            .on_click(cx.listener(|_, checked: &bool, _, cx| {
                                let enabled = *checked;
                                cx.update_global::<AppState, _>(move |s, _| {
                                    s.set_check_for_updates(enabled);
                                });
                                cx.notify();
                            })),
                        "检查更新",
                        "每次启动检查一次新版本(仅查询,不自动下载)。",
                        pal,
                    )),
            )
    }
}

/// One row: title + muted description on the left, the control on the right.
fn setting_row(
    control: Switch,
    title: &'static str,
    description: &'static str,
    pal: Palette,
) -> impl IntoElement {
    h_flex()
        .w_full()
        .items_center()
        .justify_between()
        .gap_4()
        .child(
            v_flex().gap_1().child(div().text_sm().child(title)).child(
                div()
                    .text_xs()
                    .text_color(pal.text_muted)
                    .child(description),
            ),
        )
        .child(control)
}
