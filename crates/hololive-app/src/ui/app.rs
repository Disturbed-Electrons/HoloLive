use std::{
    sync::Arc,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use eframe::egui;
use hololive_core::{Precision, next_fast_len};
use temscript_client::TemscriptClient;
use temscript_simulator::SimulatorHandle;

use crate::{
    AppConfig,
    pipeline::{
        CameraSettings, FrameSample, LiveResult, Pipeline, PipelineEvent, ReconstructionCommand,
        RuntimeStatus,
    },
};

use super::{
    AGE_REFRESH_INTERVAL, LiveView, Notice, NoticeKind, controls, footer, header, shortcuts,
    texture, viewport, window,
};

pub(crate) fn run_gui(
    precision: Precision,
    client: TemscriptClient,
    simulator: Option<SimulatorHandle>,
    config: AppConfig,
) -> eframe::Result {
    match precision {
        Precision::F32 => run_typed::<f32>(precision, client, simulator, config),
        Precision::F64 => run_typed::<f64>(precision, client, simulator, config),
    }
}

fn run_typed<T: FrameSample>(
    precision: Precision,
    client: TemscriptClient,
    simulator: Option<SimulatorHandle>,
    config: AppConfig,
) -> eframe::Result {
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_title("HoloLive")
            .with_app_id("HoloLive")
            .with_inner_size([1_100.0, 760.0])
            .with_resizable(false),
        ..Default::default()
    };
    eframe::run_native(
        "HoloLive",
        options,
        Box::new(move |creation| {
            Ok(Box::new(HoloLiveApp::<T>::new(
                creation, precision, client, simulator, config,
            )))
        }),
    )
}

pub(crate) struct HoloLiveApp<T: FrameSample> {
    pub(crate) pipeline: Pipeline<T>,
    /// Frame-local copy of pipeline status. Refreshed once per `logic()` tick;
    /// discrete actions update it immediately so later widgets see the change.
    pub(crate) status: RuntimeStatus,
    pub(crate) simulator: Option<SimulatorHandle>,
    pub(crate) config: AppConfig,
    pub(crate) precision: Precision,
    pub(crate) view: LiveView,
    pub(crate) view_changed_at: Instant,
    pub(crate) result: Option<LiveResult>,
    pub(crate) live: texture::LiveTexture,
    pub(crate) notice: Notice,
    seen_reference_revision: u64,
    pub(crate) displayed_age_ms: f64,
}

impl<T: FrameSample> HoloLiveApp<T> {
    fn new(
        creation: &eframe::CreationContext<'_>,
        precision: Precision,
        client: TemscriptClient,
        simulator: Option<SimulatorHandle>,
        config: AppConfig,
    ) -> Self {
        window::configure_style(&creation.egui_ctx);
        window::size_window_to_display(&creation.egui_ctx);
        let ctx = creation.egui_ctx.clone();
        let pipeline = Pipeline::start(
            Arc::new(move || ctx.request_repaint()),
            client,
            CameraSettings {
                camera: config.camera.clone(),
                exposure_seconds: config.exposure_seconds,
                binning: config.binning,
            },
            config.reconstruction_settings(),
        );
        let status = pipeline.status();
        Self {
            pipeline,
            status,
            simulator,
            config,
            precision,
            view: LiveView::Hologram,
            view_changed_at: Instant::now(),
            result: None,
            live: texture::LiveTexture::default(),
            notice: Notice::default(),
            seen_reference_revision: 0,
            displayed_age_ms: 0.0,
        }
    }

    fn receive_result(&mut self) {
        if let Some(next) = self.pipeline.take_result() {
            let was_unwrapped = self
                .result
                .as_ref()
                .and_then(|result| result.reconstruction)
                .is_some_and(|result| result.unwrapped_range.is_some());
            let now_unwrapped = next
                .reconstruction
                .is_some_and(|result| result.unwrapped_range.is_some());
            if let Some(previous) = self.result.replace(next) {
                self.pipeline.recycle(previous);
            }
            if now_unwrapped && !was_unwrapped {
                self.notice.show(
                    NoticeKind::Status,
                    "Phase Unwrapping Using Arevalillo–Herráez (2002)",
                );
            }
            self.live.mark_dirty();
        }
        if let Some(event) = self.pipeline.take_event() {
            self.apply_pipeline_event(event);
        }
        if let Some(result) = self.current_reconstruction() {
            self.config.crop.remember_automatic(result.crop_radius);
        }
    }

    fn refresh_status(&mut self) {
        let previous_size = (self.status.width, self.status.height);
        self.status = self.pipeline.status();
        let frame_size = (self.status.width, self.status.height);
        if !self.status.supported_binnings.is_empty()
            && self.config.supported_binnings != self.status.supported_binnings
        {
            self.config
                .apply_supported_binnings(self.status.supported_binnings.clone())
                .expect("acquisition validated camera binnings");
        }
        if let Some(capture) = self.status.last_reference_capture
            && capture.revision != self.seen_reference_revision
        {
            self.seen_reference_revision = capture.revision;
            if self.status.reference_active && capture.frame_size == frame_size {
                self.config.crop.set_manual(capture.crop_radius);
                self.config.reconstruction.locked_sideband = Some(capture.sideband);
                self.notice
                    .show(NoticeKind::Status, "Reference Captured; Sideband Locked");
            }
        }
        if previous_size.0 != 0 && previous_size != frame_size {
            self.config.reconstruction.locked_sideband = None;
            self.config.crop = Default::default();
            self.pipeline
                .reconstruction(ReconstructionCommand::ClearReference);
            self.send_settings();
        }
    }

    pub(crate) fn send_settings(&mut self) {
        let settings = self.config.reconstruction_settings();
        if let Err(error) = settings.validate() {
            self.notice
                .show(NoticeKind::Status, format!("Invalid Settings: {error}"));
            return;
        }
        self.pipeline
            .reconstruction(ReconstructionCommand::Settings(settings));
    }

    pub(crate) fn set_view(&mut self, view: LiveView) {
        if self.view == view {
            return;
        }
        self.view = view;
        self.view_changed_at = Instant::now();
        self.pipeline
            .reconstruction(ReconstructionCommand::View(view));
    }

    pub(crate) fn lock_current_sideband(&mut self) {
        let Some(result) = self.current_reconstruction() else {
            return;
        };
        let sideband = result.sideband;
        self.config.crop.set_manual(result.crop_radius);
        self.config.reconstruction.locked_sideband = Some(sideband);
    }

    pub(crate) fn toggle_sideband_lock(&mut self) -> bool {
        if self.config.reconstruction.locked_sideband.is_some() {
            self.unlock_sideband();
            self.notice.show(NoticeKind::Status, "Sideband Unlocked");
            true
        } else if self.current_reconstruction().is_some() {
            self.lock_current_sideband();
            self.notice.show(NoticeKind::Status, "Sideband Locked");
            true
        } else {
            false
        }
    }

    pub(crate) fn unlock_sideband(&mut self) -> bool {
        if self.config.reconstruction.locked_sideband.is_none() {
            return false;
        }
        self.config.reconstruction.locked_sideband = None;
        if self.status.reference_active {
            self.pipeline
                .reconstruction(ReconstructionCommand::ClearReference);
        }
        true
    }

    pub(crate) fn sync_output_size(
        &mut self,
        detector_width: usize,
        detector_height: usize,
    ) -> bool {
        if detector_width == 0 || detector_height == 0 {
            return false;
        }
        let requested = ((detector_width.max(detector_height) as f32 / self.config.output_divisor)
            .floor() as usize)
            .max(16);
        let crop_floor = self
            .current_reconstruction()
            .map_or(16, |result| result.crop_radius.saturating_mul(2));
        let output = next_fast_len(requested.max(crop_floor));
        if self.config.reconstruction.output_width == output
            && self.config.reconstruction.output_height == output
        {
            return false;
        }
        self.config.reconstruction.output_width = output;
        self.config.reconstruction.output_height = output;
        true
    }

    fn capture_reference(&mut self) {
        if self.current_reconstruction().is_none() {
            self.notice.show(
                NoticeKind::Status,
                "No Reconstruction Available For Reference Capture",
            );
            return;
        }
        self.pipeline
            .reconstruction(ReconstructionCommand::CaptureReference);
    }

    pub(crate) fn clear_reference(&mut self) {
        self.pipeline
            .reconstruction(ReconstructionCommand::ClearReference);
        self.notice.show(NoticeKind::Status, "Reference Cleared");
    }

    pub(crate) fn toggle_reference(&mut self) {
        if self.status.reference_active {
            self.clear_reference();
        } else {
            self.capture_reference();
        }
    }

    pub(crate) fn save_current(&mut self) {
        if self.current_reconstruction().is_none() {
            self.notice.show(NoticeKind::Status, "Nothing To Save Yet");
            return;
        }
        self.pipeline.reconstruction(ReconstructionCommand::Save);
        self.notice.show(NoticeKind::Status, "Saving PNG…");
    }

    pub(crate) fn current_reconstruction(&self) -> Option<hololive_core::ReconstructionInfo> {
        self.result
            .as_ref()
            .and_then(|result| result.reconstruction)
    }

    fn apply_pipeline_event(&mut self, event: PipelineEvent) {
        match event {
            PipelineEvent::ReconstructionFailed(error) => {
                self.notice
                    .show(NoticeKind::Status, format!("Reconstruction: {error}"));
            }
            PipelineEvent::CameraConfigureFailed(error) => {
                self.notice.show(NoticeKind::Camera, error);
            }
            PipelineEvent::SaveFinished(Ok(message)) => {
                self.notice.show(NoticeKind::Status, message);
            }
            PipelineEvent::SaveFinished(Err(error)) => {
                self.notice
                    .show(NoticeKind::Status, format!("Save Failed: {error}"));
            }
        }
    }
}

impl<T: FrameSample> eframe::App for HoloLiveApp<T> {
    fn logic(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        self.receive_result();
        self.refresh_status();
        if self.sync_output_size(self.status.width, self.status.height) {
            self.send_settings();
        }
        shortcuts::keyboard_shortcuts(self, context);
        if let Some(result) = &self.result {
            self.displayed_age_ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
                .saturating_sub(result.acquired_unix_ns) as f64
                / 1_000_000.0;
        }
        self.notice.expire();
        context.request_repaint_after(AGE_REFRESH_INTERVAL);
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        header::header(self, ui);
        footer::footer(self, ui);
        controls::controls(self, ui);
        self.live.upload(
            frame,
            self.result.as_ref().map(|result| {
                (
                    result.rgba.as_slice(),
                    result.display_width,
                    result.display_height,
                    result.nearest,
                )
            }),
        );
        viewport::viewport(self, ui);
    }
}
