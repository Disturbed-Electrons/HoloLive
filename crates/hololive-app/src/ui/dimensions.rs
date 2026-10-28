pub(crate) const HEADER_HEIGHT: f32 = 48.0;
pub(crate) const FOOTER_HEIGHT: f32 = 28.0;
pub(crate) const CONTROL_PANEL_WIDTH: f32 = 316.0;
pub(crate) const TOGGLE_BUTTON_SIZE: f32 = 30.0;
pub(crate) const CONTROL_ROW_GAP: f32 = 8.0;
pub(crate) const REGION_COMBO_WIDTH: f32 = 46.0;
pub(crate) const SLIDER_VALUE_WIDTH: f32 = 48.0;
pub(crate) const SLIDER_LABEL_WIDTH: f32 = 60.0;
pub(crate) const HEADER_RIGHT_BUTTON_GROUP_WIDTH: f32 = 286.0;
/// UI heartbeat: labels, connection state, and frame age stay live at ~60 Hz
/// even when acquisition or reconstruction is slower (or faster) than the display.
pub(crate) const AGE_REFRESH_INTERVAL: std::time::Duration =
    std::time::Duration::from_nanos(16_666_667);
