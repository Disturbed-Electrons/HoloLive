mod acquisition;
mod camera;
mod latest;
mod live;
mod reconstruction;
mod status;

use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::Duration,
};

use crossbeam_channel::{Sender, unbounded};
use hololive_core::{Frame, ReconstructionSettings};
use temscript_client::TemscriptClient;

pub(crate) use live::{AcquisitionCommand, LiveView};
pub use live::{CameraSettings, FrameSample, LiveResult, ReconstructionCommand};
pub(crate) use status::{ConnectionState, PipelineEvent, RuntimeStatus};

use acquisition::{AcquisitionWorker, spawn_acquisition};
use camera::{arm_panic_reset, disarm_panic_reset, install_panic_reset_hook, reset_camera};
use latest::LatestSlot;
use reconstruction::{ReconstructionWorker, spawn_reconstruction};

pub struct Pipeline<T> {
    frames: Arc<LatestSlot<Frame<T>>>,
    results: Arc<LatestSlot<LiveResult>>,
    recycle: Sender<LiveResult>,
    acquisition_commands: Arc<LatestSlot<AcquisitionCommand>>,
    reconstruction_commands: Sender<ReconstructionCommand>,
    reconstruction_settings: Arc<LatestSlot<ReconstructionSettings>>,
    events: Arc<LatestSlot<PipelineEvent>>,
    status: Arc<Mutex<RuntimeStatus>>,
    paused: Arc<AtomicBool>,
    cancelled: Arc<AtomicBool>,
    client: TemscriptClient,
    configured_camera: Arc<Mutex<Option<String>>>,
    acquisition_worker: Option<JoinHandle<()>>,
    reconstruction_worker: Option<JoinHandle<()>>,
}

impl<T: FrameSample> Pipeline<T> {
    pub fn start(
        repaint: Arc<dyn Fn() + Send + Sync>,
        client: TemscriptClient,
        camera: CameraSettings,
        settings: ReconstructionSettings,
    ) -> Self {
        let frames = Arc::new(LatestSlot::new());
        let results = Arc::new(LatestSlot::new());
        let events = Arc::new(LatestSlot::new());
        let status = Arc::new(Mutex::new(RuntimeStatus::new(camera.camera.clone())));
        let paused = Arc::new(AtomicBool::new(false));
        let cancelled = Arc::new(AtomicBool::new(false));
        let configured_camera = Arc::new(Mutex::new(None));
        let acquisition_commands = Arc::new(LatestSlot::new());
        let reconstruction_settings = Arc::new(LatestSlot::new());
        let (reconstruction_tx, reconstruction_rx) = unbounded();
        let (recycle_tx, recycle_rx) = unbounded();

        install_panic_reset_hook();
        arm_panic_reset(client.clone(), camera.camera.clone());

        let acquisition = spawn_acquisition(AcquisitionWorker {
            client: client.clone(),
            camera: camera.camera.into(),
            exposure_seconds: camera.exposure_seconds,
            binning: camera.binning,
            frames: Arc::clone(&frames),
            commands: Arc::clone(&acquisition_commands),
            events: Arc::clone(&events),
            status: Arc::clone(&status),
            paused: Arc::clone(&paused),
            cancelled: Arc::clone(&cancelled),
            configured_camera: Arc::clone(&configured_camera),
            pixels: Vec::new(),
        });
        let reconstruction = spawn_reconstruction(ReconstructionWorker {
            repaint,
            frames: Arc::clone(&frames),
            results: Arc::clone(&results),
            settings: Arc::clone(&reconstruction_settings),
            commands: reconstruction_rx,
            recycle: recycle_rx,
            events: Arc::clone(&events),
            status: Arc::clone(&status),
            paused: Arc::clone(&paused),
            cancelled: Arc::clone(&cancelled),
            initial_settings: settings,
        });
        Self {
            frames,
            results,
            recycle: recycle_tx,
            acquisition_commands,
            reconstruction_commands: reconstruction_tx,
            reconstruction_settings,
            events,
            status,
            paused,
            cancelled,
            client,
            configured_camera,
            acquisition_worker: Some(acquisition),
            reconstruction_worker: Some(reconstruction),
        }
    }
}

impl<T> Pipeline<T> {
    pub(crate) fn status(&self) -> RuntimeStatus {
        self.status.lock().expect("status lock poisoned").clone()
    }

    pub(crate) fn take_event(&self) -> Option<PipelineEvent> {
        self.events.take()
    }

    pub fn take_result(&self) -> Option<LiveResult> {
        self.results.take()
    }

    pub fn recycle(&self, result: LiveResult) {
        let _ = self.recycle.send(result);
    }

    pub(crate) fn paused(&self) -> bool {
        self.paused.load(Ordering::Acquire)
    }

    pub(crate) fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::Release);
    }

    pub(crate) fn acquisition(&self, command: AcquisitionCommand) {
        let _superseded = self.acquisition_commands.publish(command);
    }

    pub fn reconstruction(&self, command: ReconstructionCommand) {
        match command {
            ReconstructionCommand::Settings(settings) => {
                let _superseded = self.reconstruction_settings.publish(settings);
            }
            command => {
                let _ = self.reconstruction_commands.send(command);
            }
        }
        self.frames.wake();
    }
}

impl<T> Drop for Pipeline<T> {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        self.frames.wake();
        if let Some(worker) = self.reconstruction_worker.take() {
            let _ = worker.join();
        }
        if let Some(worker) = self.acquisition_worker.take() {
            let _ = worker.join();
        }
        reset_camera(
            &self.client,
            &self.configured_camera,
            Some(Duration::from_secs(1)),
        );
        disarm_panic_reset();
    }
}
