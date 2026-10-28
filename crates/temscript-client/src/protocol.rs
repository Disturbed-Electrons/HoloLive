use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const VERSION: &str = "2.1.0";
pub const VERSION_PATH: &str = "/v1/version";
pub const CAMERAS_PATH: &str = "/v1/cameras";
pub const ACQUIRE_PATH: &str = "/v1/acquire";
pub const CAMERA_PARAM_PREFIX: &str = "/v1/camera_param/";

#[must_use]
pub fn camera_param_path(camera: &str) -> String {
    format!("{CAMERA_PARAM_PREFIX}{}", urlencoding::encode(camera))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CameraInfo {
    #[serde(rename = "type")]
    pub detector_type: String,
    pub width: usize,
    pub height: usize,
    #[serde(default)]
    pub binnings: Vec<u32>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CameraParameters(pub BTreeMap<String, Value>);

impl CameraParameters {
    /// Camera parameters for a readout. Exposure is always included with
    /// binning because setting binning on some CCD cameras (Titan 1.1 /
    /// Gatan US1000) overwrites the exposure time.
    #[must_use]
    pub fn acquisition(exposure_seconds: f64, binning: u32) -> Self {
        Self(BTreeMap::from([
            ("binning".into(), Value::from(binning)),
            ("exposure(s)".into(), Value::from(exposure_seconds)),
        ]))
    }

    #[must_use]
    pub fn exposure(exposure_seconds: f64) -> Self {
        Self(BTreeMap::from([(
            "exposure(s)".into(),
            Value::from(exposure_seconds),
        )]))
    }
}
