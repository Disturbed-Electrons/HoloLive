use hololive_core::{ReconstructionInfo, ReconstructionSettings};
use temscript_client::DecodedArray;

#[derive(Debug, Clone)]
pub struct CameraSettings {
    pub camera: String,
    pub exposure_seconds: f64,
    pub binning: u32,
}

#[derive(Debug)]
pub struct AcquisitionCommand {
    pub exposure_seconds: f64,
    pub binning: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LiveView {
    #[default]
    Hologram,
    Spectrum,
    Reconstruction,
}

impl LiveView {
    pub const ALL: [Self; 3] = [Self::Hologram, Self::Spectrum, Self::Reconstruction];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Hologram => "Hologram",
            Self::Spectrum => "FFT Spectrum",
            Self::Reconstruction => "Reconstruction",
        }
    }
}

#[derive(Debug)]
pub enum ReconstructionCommand {
    Settings(ReconstructionSettings),
    View(LiveView),
    CaptureReference,
    ClearReference,
    Save,
}

/// Packed live frame plus reconstruction metadata for overlays. The wave itself
/// stays on the reconstruction thread until a [`ReconstructionCommand::Save`].
#[derive(Default)]
pub struct LiveResult {
    pub view: LiveView,
    pub acquired_unix_ns: u128,
    pub reconstruction: Option<ReconstructionInfo>,
    pub rgba: Vec<u8>,
    pub display_width: usize,
    pub display_height: usize,
    pub nearest: bool,
}

pub trait FrameSample: hololive_core::Real {
    fn convert_into(array: &DecodedArray, pixels: &mut Vec<Self>);
}

impl FrameSample for f32 {
    fn convert_into(array: &DecodedArray, pixels: &mut Vec<Self>) {
        array.fill_f32(pixels);
    }
}

impl FrameSample for f64 {
    fn convert_into(array: &DecodedArray, pixels: &mut Vec<Self>) {
        array.fill_f64(pixels);
    }
}
