use hidpp::protocol::v20::{ErrorType, Hidpp20Error};
use serde::{Deserialize, Serialize};
use thiserror::Error;

// Serializable + Clone so it can cross the agent↔GUI IPC unchanged: the GUI
// classifies a device read/write error as permanent (FeatureUnsupported /
// EmptyDpiList) vs transient, so the discriminating variant must survive the
// wire — stringifying it would collapse every case to "transient" and a device
// that genuinely lacks a feature would be re-probed forever. Variant order is
// therefore wire format: changes require a `PROTOCOL_VERSION` bump (guarded
// by `openlogi-agent-core/tests/wire_format.rs`).
#[derive(Debug, Clone, Error, Serialize, Deserialize)]
pub enum WriteError {
    // `async_hid::HidError` isn't `Serialize`, so carry its message as text; the
    // typed error is never matched on (only constructed + displayed).
    #[error("HID transport error: {0}")]
    Hid(String),
    #[error("no connected device matched the route")]
    DeviceNotFound,
    #[error("device at index {index:#04x} did not respond to HID++")]
    DeviceUnreachable { index: u8 },
    #[error("device does not expose HID++ feature {feature_hex:#06x}")]
    FeatureUnsupported { feature_hex: u16 },
    #[error("device returned no supported DPI values")]
    EmptyDpiList,
    #[error("HID++ protocol error: {0}")]
    Hidpp(String),
    #[error("HID++ feature error during {operation:?} for feature {feature_hex:#06x}: {kind:?}")]
    HidppFeature {
        operation: HidppOperation,
        feature_hex: u16,
        kind: HidppFeatureErrorKind,
    },
    #[error("HID++ unsupported response during {operation:?} for feature {feature_hex:#06x}")]
    UnsupportedResponse {
        operation: HidppOperation,
        feature_hex: u16,
    },
    #[error("HID++ request timed out during {operation:?}")]
    RequestTimedOut { operation: HidppOperation },
    #[error("tokio runtime init failed: {message}")]
    RuntimeInit { message: String },
    #[error("background agent is unavailable")]
    AgentUnavailable,
}

/// HID++ operation being performed when a device write/read failed.
///
/// Variant order is wire format because this travels inside [`WriteError`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HidppOperation {
    ResolveFeature,
    DumpFeatures,
    ReadDpi,
    ReadDpiCapabilities,
    WriteDpi,
    ReadSmartShift,
    WriteSmartShift,
    Lighting,
}

/// HID++ feature error kind in a serializable wire-safe form.
///
/// Variant order is wire format because this travels inside [`WriteError`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HidppFeatureErrorKind {
    NoError,
    Unknown,
    InvalidArgument,
    OutOfRange,
    HwError,
    LogitechInternal,
    InvalidFeatureIndex,
    InvalidFunctionId,
    Busy,
    Unsupported,
    Unrecognized,
}

impl From<async_hid::HidError> for WriteError {
    fn from(e: async_hid::HidError) -> Self {
        Self::Hid(e.to_string())
    }
}

fn hidpp_feature_error_kind(kind: ErrorType) -> HidppFeatureErrorKind {
    match kind {
        ErrorType::NoError => HidppFeatureErrorKind::NoError,
        ErrorType::Unknown => HidppFeatureErrorKind::Unknown,
        ErrorType::InvalidArgument => HidppFeatureErrorKind::InvalidArgument,
        ErrorType::OutOfRange => HidppFeatureErrorKind::OutOfRange,
        ErrorType::HwError => HidppFeatureErrorKind::HwError,
        ErrorType::LogitechInternal => HidppFeatureErrorKind::LogitechInternal,
        ErrorType::InvalidFeatureIndex => HidppFeatureErrorKind::InvalidFeatureIndex,
        ErrorType::InvalidFunctionId => HidppFeatureErrorKind::InvalidFunctionId,
        ErrorType::Busy => HidppFeatureErrorKind::Busy,
        ErrorType::Unsupported => HidppFeatureErrorKind::Unsupported,
        _ => HidppFeatureErrorKind::Unrecognized,
    }
}

pub(crate) fn classify_hidpp_error(
    error: Hidpp20Error,
    operation: HidppOperation,
    feature_hex: u16,
) -> WriteError {
    match error {
        Hidpp20Error::Feature(kind) => WriteError::HidppFeature {
            operation,
            feature_hex,
            kind: hidpp_feature_error_kind(kind),
        },
        Hidpp20Error::UnsupportedResponse => WriteError::UnsupportedResponse {
            operation,
            feature_hex,
        },
        Hidpp20Error::Channel(error) => WriteError::Hidpp(format!("{error:?}")),
        _ => WriteError::Hidpp(format!("{error:?}")),
    }
}
