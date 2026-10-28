use crate::{
    CoreError,
    sideband::{SearchRegion, Sideband},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DisplayMode {
    #[default]
    Phase,
    Amplitude,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReconstructionSettings {
    pub output_width: usize,
    pub output_height: usize,
    pub search_region: SearchRegion,
    pub center_mask_percent: f32,
    pub locked_sideband: Option<Sideband>,
    pub crop_radius: Option<usize>,
    pub tukey_enabled: bool,
    pub tukey_alpha: f32,
    pub display_mode: DisplayMode,
    pub phase_amplification: u8,
    pub phase_unwrap: bool,
}

impl Default for ReconstructionSettings {
    fn default() -> Self {
        Self {
            output_width: 512,
            output_height: 512,
            search_region: SearchRegion::Upper,
            center_mask_percent: 2.0,
            locked_sideband: None,
            crop_radius: None,
            tukey_enabled: true,
            tukey_alpha: 0.2,
            display_mode: DisplayMode::Phase,
            phase_amplification: 1,
            phase_unwrap: false,
        }
    }
}

impl ReconstructionSettings {
    pub fn validate(&self) -> Result<(), CoreError> {
        crate::checked_len(self.output_width, self.output_height)?;
        if !self.center_mask_percent.is_finite() || !(0.0..=9.9).contains(&self.center_mask_percent)
        {
            return Err(CoreError::InvalidSetting(
                "center mask must be in [0, 9.9] percent",
            ));
        }
        if !(1..=4).contains(&self.phase_amplification) {
            return Err(CoreError::InvalidSetting(
                "phase amplification must be in [1, 4]",
            ));
        }
        Ok(())
    }
}
