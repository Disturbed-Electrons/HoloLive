use std::sync::Mutex;

use hololive_core::{CoreError, Sideband};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Connecting,
    Connected,
    Reconnecting,
    Disconnected,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PipelineEvent {
    ReconstructionFailed(CoreError),
    CameraConfigureFailed(String),
    SaveFinished(Result<String, String>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ReferenceCapture {
    pub revision: u64,
    pub sideband: Sideband,
    pub crop_radius: usize,
    pub frame_size: (usize, usize),
}

#[derive(Debug, Clone)]
pub struct RuntimeStatus {
    pub connection: ConnectionState,
    pub camera: String,
    pub width: usize,
    pub height: usize,
    pub acquisition_fps: f64,
    pub acquisition_failures: u64,
    pub last_acquisition_error: Option<String>,
    pub supported_binnings: Vec<u32>,
    pub reference_active: bool,
    pub last_reference_capture: Option<ReferenceCapture>,
    reference_revision: u64,
}

impl RuntimeStatus {
    pub(crate) fn new(camera: String) -> Self {
        Self {
            connection: ConnectionState::Connecting,
            camera,
            width: 0,
            height: 0,
            acquisition_fps: 0.0,
            acquisition_failures: 0,
            last_acquisition_error: None,
            supported_binnings: Vec::new(),
            reference_active: false,
            last_reference_capture: None,
            reference_revision: 0,
        }
    }

    pub(crate) fn mark_connected(&mut self) {
        self.connection = ConnectionState::Connected;
    }

    pub(crate) fn capture_reference(
        &mut self,
        sideband: Sideband,
        crop_radius: usize,
        frame_size: (usize, usize),
    ) {
        self.reference_revision = self.reference_revision.wrapping_add(1);
        self.reference_active = true;
        self.last_reference_capture = Some(ReferenceCapture {
            revision: self.reference_revision,
            sideband,
            crop_radius,
            frame_size,
        });
    }

    pub(crate) fn clear_reference(&mut self) {
        self.reference_active = false;
    }
}

pub(crate) fn set_error(status: &Mutex<RuntimeStatus>, state: ConnectionState, message: String) {
    let mut current = status.lock().expect("status lock poisoned");
    current.connection = state;
    current.last_acquisition_error = Some(message);
}

pub(crate) fn record_acquisition_failure(status: &Mutex<RuntimeStatus>, message: String) {
    let mut current = status.lock().expect("status lock poisoned");
    current.connection = ConnectionState::Reconnecting;
    current.acquisition_failures += 1;
    current.last_acquisition_error = Some(message);
}
