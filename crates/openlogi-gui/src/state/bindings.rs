//! Binding overlay helpers for [`super::AppState`].

use std::collections::BTreeMap;

use openlogi_core::config::Config;

use crate::data::mouse_buttons::{
    Action, ButtonId, GestureDirection, default_binding, default_gesture_binding,
};
use crate::state::DeviceRecord;

pub(super) fn bindings_for(
    config: &Config,
    record: Option<&DeviceRecord>,
    app_bundle: Option<&str>,
) -> BTreeMap<ButtonId, Action> {
    let stored = record
        .map(|r| config.effective_bindings(&r.config_key, app_bundle))
        .unwrap_or_default();
    let mut bindings: BTreeMap<ButtonId, Action> = ButtonId::ALL
        .iter()
        .copied()
        .map(|b| (b, default_binding(b)))
        .collect();
    for (k, v) in stored {
        bindings.insert(k, v);
    }
    bindings
}

pub(super) fn gesture_bindings_for(
    config: &Config,
    record: Option<&DeviceRecord>,
) -> BTreeMap<GestureDirection, Action> {
    let stored = record
        .map(|r| config.gesture_bindings_for(&r.config_key))
        .unwrap_or_default();
    let mut bindings: BTreeMap<GestureDirection, Action> = GestureDirection::ALL
        .iter()
        .copied()
        .map(|d| (d, default_gesture_binding(d)))
        .collect();
    for (k, v) in stored {
        bindings.insert(k, v);
    }
    bindings
}
