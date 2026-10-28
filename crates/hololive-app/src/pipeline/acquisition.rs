use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use hololive_core::{Frame, FrameMeta};
use temscript_client::TemscriptClient;
use tracing::warn;

use super::{
    AcquisitionCommand, FrameSample,
    camera::{configure_camera, initialize_camera},
    latest::LatestSlot,
    status::{
        ConnectionState, PipelineEvent, RuntimeStatus, record_acquisition_failure, set_error,
    },
};

pub(crate) struct AcquisitionWorker<T> {
    pub client: TemscriptClient,
    pub camera: Arc<str>,
    pub exposure_seconds: f64,
    pub binning: u32,
    pub frames: Arc<LatestSlot<Frame<T>>>,
    pub commands: Arc<LatestSlot<AcquisitionCommand>>,
    pub events: Arc<LatestSlot<PipelineEvent>>,
    pub status: Arc<Mutex<RuntimeStatus>>,
    pub paused: Arc<AtomicBool>,
    pub cancelled: Arc<AtomicBool>,
    pub configured_camera: Arc<Mutex<Option<String>>>,
    pub pixels: Vec<T>,
}

const INITIAL_BACKOFF: Duration = Duration::from_millis(200);

pub(crate) fn spawn_acquisition<T: FrameSample>(
    mut worker: AcquisitionWorker<T>,
) -> JoinHandle<()> {
    thread::Builder::new()
        .name("hololive-acquisition".into())
        .spawn(move || worker.run())
        .expect("failed to start acquisition thread")
}

impl<T: FrameSample> AcquisitionWorker<T> {
    fn run(&mut self) {
        let mut camera_ready = false;
        let mut sequence = 0;
        let mut previous = Instant::now();
        let mut backoff = INITIAL_BACKOFF;
        while !self.cancelled.load(Ordering::Acquire) {
            camera_ready = self.apply_configure(camera_ready);
            if self.paused() {
                thread::sleep(Duration::from_millis(20));
                continue;
            }
            if !camera_ready {
                match initialize_camera(
                    &self.client,
                    &self.camera,
                    self.exposure_seconds,
                    self.binning,
                    &self.status,
                    &self.configured_camera,
                ) {
                    Ok(binning) => {
                        self.binning = binning;
                        camera_ready = true;
                        backoff = INITIAL_BACKOFF;
                    }
                    Err(message) => {
                        set_error(&self.status, ConnectionState::Reconnecting, message);
                        self.retry_with_backoff(&mut backoff);
                        continue;
                    }
                }
            }
            let frame_exposure = self.exposure_seconds;
            let frame_binning = self.binning;
            match self.client.acquire(&self.camera) {
                Ok(array) => {
                    camera_ready = self.apply_configure(camera_ready);
                    if !camera_ready {
                        continue;
                    }
                    if self.paused() {
                        continue;
                    }
                    let now = Instant::now();
                    let instant_fps = 1.0 / now.duration_since(previous).as_secs_f64().max(1e-6);
                    previous = now;
                    let width = array.width;
                    let height = array.height;
                    T::convert_into(&array, &mut self.pixels);
                    let frame = Frame::new(
                        width,
                        height,
                        std::mem::take(&mut self.pixels),
                        FrameMeta {
                            sequence,
                            acquired_unix_ns: SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_nanos(),
                            detector: Arc::clone(&self.camera),
                            exposure_seconds: frame_exposure,
                            binning: frame_binning,
                        },
                    )
                    .expect("validated protocol array has matching dimensions");
                    if let Some(previous) = self.frames.publish(frame) {
                        self.pixels = previous.into_pixels();
                    }
                    let mut current = self.status.lock().expect("status lock poisoned");
                    current.mark_connected();
                    current.width = width;
                    current.height = height;
                    current.acquisition_fps = ema_fps(current.acquisition_fps, instant_fps);
                    drop(current);
                    sequence += 1;
                    backoff = INITIAL_BACKOFF;
                }
                Err(error) => {
                    if self.cancelled.load(Ordering::Acquire) {
                        break;
                    }
                    warn!(%error, "acquisition failed");
                    record_acquisition_failure(&self.status, error.to_string());
                    camera_ready = false;
                    self.retry_with_backoff(&mut backoff);
                }
            }
        }
        self.status.lock().expect("status lock poisoned").connection =
            ConnectionState::Disconnected;
    }

    fn paused(&self) -> bool {
        self.paused.load(Ordering::Acquire)
    }

    fn apply_configure(&mut self, camera_ready: bool) -> bool {
        let Some(command) = self.commands.take() else {
            return camera_ready;
        };
        self.exposure_seconds = command.exposure_seconds;
        self.binning = command.binning;
        if !camera_ready {
            return false;
        }
        match configure_camera(
            &self.client,
            &self.camera,
            self.exposure_seconds,
            self.binning,
            None,
        ) {
            Ok(()) => true,
            Err(error) => {
                let _superseded = self
                    .events
                    .publish(PipelineEvent::CameraConfigureFailed(error.to_string()));
                false
            }
        }
    }

    fn retry_with_backoff(&mut self, backoff: &mut Duration) {
        self.sleep_backoff(*backoff);
        *backoff = (*backoff * 2).min(Duration::from_secs(3));
    }

    fn sleep_backoff(&mut self, backoff: Duration) {
        let end = Instant::now() + backoff;
        while !self.cancelled.load(Ordering::Acquire) && Instant::now() < end && !self.paused() {
            let _ = self.apply_configure(false);
            if self.paused() {
                break;
            }
            thread::sleep(
                Duration::from_millis(20).min(end.saturating_duration_since(Instant::now())),
            );
        }
    }
}

fn ema_fps(previous: f64, current: f64) -> f64 {
    if previous == 0.0 {
        current
    } else {
        previous.mul_add(0.9, current * 0.1)
    }
}
