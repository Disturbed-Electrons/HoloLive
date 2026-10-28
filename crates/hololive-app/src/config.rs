use hololive_core::ReconstructionSettings;
use temscript_client::CameraInfo;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CropRadius {
    Automatic { last_manual: usize },
    Manual(usize),
}

impl CropRadius {
    pub fn last_manual(self) -> usize {
        match self {
            Self::Automatic { last_manual } | Self::Manual(last_manual) => last_manual.max(1),
        }
    }

    pub fn is_automatic(self) -> bool {
        matches!(self, Self::Automatic { .. })
    }

    pub fn to_settings(self) -> Option<usize> {
        match self {
            Self::Automatic { .. } => None,
            Self::Manual(radius) => Some(radius.max(1)),
        }
    }

    pub fn toggle_mode(&mut self) {
        *self = match *self {
            Self::Automatic { last_manual } => Self::Manual(last_manual.max(1)),
            Self::Manual(last_manual) => Self::Automatic { last_manual },
        };
    }

    pub fn set_manual(&mut self, radius: usize) {
        *self = Self::Manual(radius.max(1));
    }

    pub fn remember_automatic(&mut self, radius: usize) {
        if let Self::Automatic { last_manual } = self {
            *last_manual = radius.max(1);
        }
    }
}

impl Default for CropRadius {
    fn default() -> Self {
        Self::Automatic { last_manual: 8 }
    }
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub live_server: String,
    pub camera: String,
    pub exposure_seconds: f64,
    pub binning: u32,
    pub supported_binnings: Vec<u32>,
    pub output_divisor: f32,
    pub crop: CropRadius,
    pub reconstruction: ReconstructionSettings,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            live_server: "http://192.168.99.10:8080".into(),
            camera: "CCD".into(),
            exposure_seconds: 0.2,
            binning: 4,
            supported_binnings: vec![1, 2, 4, 8],
            output_divisor: 1.0,
            crop: CropRadius::default(),
            reconstruction: ReconstructionSettings::default(),
        }
    }
}

impl AppConfig {
    pub fn apply_camera_info(&mut self, camera: &CameraInfo) -> Result<(), &'static str> {
        self.apply_supported_binnings(supported_binnings(camera))
    }

    pub(crate) fn apply_supported_binnings(
        &mut self,
        binnings: Vec<u32>,
    ) -> Result<(), &'static str> {
        let selected =
            closest_binning(&binnings, self.binning).ok_or("camera advertises no binning modes")?;
        self.binning = selected;
        self.supported_binnings = binnings;
        Ok(())
    }

    pub fn reconstruction_settings(&self) -> ReconstructionSettings {
        let mut settings = self.reconstruction.clone();
        settings.crop_radius = self.crop.to_settings();
        settings
    }
}

pub(crate) fn supported_binnings(camera: &CameraInfo) -> Vec<u32> {
    let mut binnings: Vec<_> = camera
        .binnings
        .iter()
        .copied()
        .filter(|&binning| binning > 0)
        .collect();
    binnings.sort_unstable();
    binnings.dedup();
    binnings
}

pub(crate) fn closest_binning(supported: &[u32], preferred: u32) -> Option<u32> {
    supported
        .iter()
        .copied()
        .min_by_key(|&binning| (binning.abs_diff(preferred), std::cmp::Reverse(binning)))
}
