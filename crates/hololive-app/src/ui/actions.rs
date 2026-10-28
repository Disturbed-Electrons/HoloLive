use hololive_core::{DisplayMode, SearchRegion};

use crate::{AppConfig, pipeline::FrameSample, ui::HoloLiveApp};

/// Discrete UI intents shared by keyboard shortcuts and widgets.
///
/// Sliders and other continuous controls still write `AppConfig` directly;
/// anything a key and a button can both do goes through [`HoloLiveApp::apply`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Action {
    SetDisplayMode(DisplayMode),
    CycleDisplayMode,
    SetPhaseAmplification(u8),
    CyclePhaseAmplification,
    SetPhaseUnwrap(bool),
    TogglePhaseUnwrap,
    ToggleTukey,
    SetSearchRegion(SearchRegion),
    TogglePause,
    ToggleReference,
    ToggleSidebandLock,
    Save,
}

impl<T: FrameSample> HoloLiveApp<T> {
    /// Apply a discrete user intent.
    ///
    /// Returns whether reconstruction settings changed and should be sent.
    #[must_use]
    pub(crate) fn apply(&mut self, action: Action) -> bool {
        match action {
            Action::SetDisplayMode(mode) => set_display_mode(&mut self.config, mode),
            Action::CycleDisplayMode => {
                let mode = match self.config.reconstruction.display_mode {
                    DisplayMode::Phase => DisplayMode::Amplitude,
                    DisplayMode::Amplitude => DisplayMode::Phase,
                };
                set_display_mode(&mut self.config, mode)
            }
            Action::SetPhaseAmplification(amplification) => {
                set_phase_amplification(&mut self.config, amplification)
            }
            Action::CyclePhaseAmplification => {
                let amplification = self.config.reconstruction.phase_amplification % 4 + 1;
                set_phase_amplification(&mut self.config, amplification)
            }
            Action::SetPhaseUnwrap(enabled) => set_phase_unwrap(&mut self.config, enabled),
            Action::TogglePhaseUnwrap => {
                let enabled = !self.config.reconstruction.phase_unwrap;
                set_phase_unwrap(&mut self.config, enabled)
            }
            Action::ToggleTukey => {
                self.config.reconstruction.tukey_enabled =
                    !self.config.reconstruction.tukey_enabled;
                true
            }
            Action::SetSearchRegion(region) => set_search_region(&mut self.config, region),
            Action::TogglePause => {
                self.pipeline.set_paused(!self.pipeline.paused());
                false
            }
            Action::ToggleReference => {
                self.toggle_reference();
                false
            }
            Action::ToggleSidebandLock => self.toggle_sideband_lock(),
            Action::Save => {
                self.save_current();
                false
            }
        }
    }
}

fn set_display_mode(config: &mut AppConfig, mode: DisplayMode) -> bool {
    if config.reconstruction.display_mode == mode {
        return false;
    }
    config.reconstruction.display_mode = mode;
    true
}

fn set_phase_amplification(config: &mut AppConfig, amplification: u8) -> bool {
    if config.reconstruction.display_mode != DisplayMode::Phase
        || config.reconstruction.phase_unwrap
        || config.reconstruction.phase_amplification == amplification
    {
        return false;
    }
    config.reconstruction.phase_amplification = amplification;
    true
}

fn set_phase_unwrap(config: &mut AppConfig, enabled: bool) -> bool {
    if config.reconstruction.display_mode != DisplayMode::Phase
        || config.reconstruction.phase_unwrap == enabled
    {
        return false;
    }
    config.reconstruction.phase_unwrap = enabled;
    true
}

fn set_search_region(config: &mut AppConfig, region: SearchRegion) -> bool {
    if config.reconstruction.locked_sideband.is_some()
        || config.reconstruction.search_region == region
    {
        return false;
    }
    config.reconstruction.search_region = region;
    true
}
