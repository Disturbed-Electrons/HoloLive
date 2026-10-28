use std::{
    collections::BTreeMap,
    io::{Read, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use flate2::{Compression, write::GzEncoder};
use hololive_simulation::{HologramGenerator, SimulationSettings};
use rand::{RngExt as _, SeedableRng, rngs::StdRng};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use temscript_client::{
    PackedArray,
    protocol::{
        ACQUIRE_PATH, CAMERA_PARAM_PREFIX, CAMERAS_PATH, CameraInfo, CameraParameters, VERSION,
        VERSION_PATH,
    },
};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

use crate::{
    error::SimulatorError,
    simulator_config::{SIMULATED_CAMERA, SimulatorConfig, binning_divides},
};

pub struct SimulatorHandle {
    address: String,
    cancelled: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl SimulatorHandle {
    pub fn start(bind: &str, config: SimulatorConfig) -> Result<Self, SimulatorError> {
        config.validate()?;
        let server = Server::http(bind).map_err(|error| SimulatorError::Bind {
            address: bind.to_owned(),
            message: error.to_string(),
        })?;
        let address = format!("http://{}", server.server_addr());
        let cancelled = Arc::new(AtomicBool::new(false));
        let thread_cancelled = Arc::clone(&cancelled);
        let join = thread::Builder::new()
            .name("temscript-simulator".into())
            .spawn(move || serve(server, config, thread_cancelled))
            .map_err(|source| SimulatorError::Thread { source })?;
        Ok(Self {
            address,
            cancelled,
            join: Some(join),
        })
    }

    #[must_use]
    pub fn address(&self) -> &str {
        &self.address
    }
}

impl Drop for SimulatorHandle {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        if let Some(join) = self.join.take() {
            let deadline = Instant::now() + Duration::from_millis(250);
            while !join.is_finished() && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(5));
            }
            if join.is_finished() {
                let _ = join.join();
            }
        }
    }
}

struct ServeState {
    generator: HologramGenerator,
    jitter_rng: StdRng,
    frame: Vec<f64>,
    sequence: u64,
    request_count: u64,
    acquire_count: u64,
    next_frame_at: Instant,
}

impl ServeState {
    fn new(config: &SimulatorConfig) -> Self {
        let seed = config.simulation.seed;
        Self {
            generator: HologramGenerator::new(seed),
            jitter_rng: StdRng::seed_from_u64(seed ^ 0x5352_5652),
            frame: Vec::new(),
            sequence: 0,
            request_count: 0,
            acquire_count: 0,
            next_frame_at: Instant::now(),
        }
    }

    fn schedule_next_frame(&mut self, config: &SimulatorConfig) {
        let jitter = self
            .jitter_rng
            .random_range(-config.simulation.jitter_fraction..=config.simulation.jitter_fraction);
        let interval = Duration::try_from_secs_f64((1.0 + jitter) / config.simulation.target_fps)
            .expect("simulator timing was validated");
        let now = Instant::now();
        self.next_frame_at = self
            .next_frame_at
            .checked_add(interval)
            .filter(|next| *next >= now)
            .unwrap_or(now);
    }
}

fn serve(server: Server, mut config: SimulatorConfig, cancelled: Arc<AtomicBool>) {
    let mut state = ServeState::new(&config);

    while !cancelled.load(Ordering::Acquire) {
        let Ok(request) = server.recv_timeout(Duration::from_millis(50)) else {
            continue;
        };
        let Some(request) = request else { continue };

        if config.faults.extra_latency_ms > 0
            && !sleep_cancellable(
                &cancelled,
                Duration::from_millis(config.faults.extra_latency_ms),
            )
        {
            break;
        }

        state.request_count += 1;
        if inject_on_count(config.faults.http_error_every, state.request_count) {
            respond_text(request, 503, "injected simulator failure");
            continue;
        }

        let path = request_path(request.url());
        dispatch_request(request, &path, &mut config, &cancelled, &mut state);
    }
}

fn inject_on_count(every: Option<u64>, count: u64) -> bool {
    every.is_some_and(|n| n > 0 && count.is_multiple_of(n))
}

fn request_path(url: &str) -> String {
    url.split_once('?').map_or(url, |(path, _)| path).to_owned()
}

fn dispatch_request(
    request: Request,
    path: &str,
    config: &mut SimulatorConfig,
    cancelled: &AtomicBool,
    state: &mut ServeState,
) {
    match (request.method(), path) {
        (&Method::Get, VERSION_PATH) => respond_json(request, &json!(VERSION)),
        (&Method::Get, CAMERAS_PATH) => respond_cameras(request, config),
        (&Method::Get, path) if path.starts_with(CAMERA_PARAM_PREFIX) => {
            if let Some(request) = require_simulated_camera(request, camera_from_param_path(path)) {
                respond_json(request, &camera_parameters(config));
            }
        }
        (&Method::Put, path) if path.starts_with(CAMERA_PARAM_PREFIX) => {
            if let Some(request) = require_simulated_camera(request, camera_from_param_path(path)) {
                set_camera_parameters(request, config);
            }
        }
        (&Method::Get, ACQUIRE_PATH) => handle_acquire(request, config, cancelled, state),
        _ => respond_text(request, 404, "unknown temscript endpoint"),
    }
}

fn handle_acquire(
    request: Request,
    config: &SimulatorConfig,
    cancelled: &AtomicBool,
    state: &mut ServeState,
) {
    let camera = detector_from_acquire_url(request.url());
    let Some(request) = require_simulated_camera(request, camera) else {
        return;
    };
    let now = Instant::now();
    if now < state.next_frame_at && !sleep_cancellable(cancelled, state.next_frame_at - now) {
        respond_text(request, 503, "simulator shutting down");
        return;
    }

    state.acquire_count += 1;
    let generated = match config.simulation.for_binning(config.binning) {
        Ok(generated) => generated,
        Err(error) => {
            respond_text(request, 500, &error.to_string());
            return;
        }
    };
    match state
        .generator
        .generate(&generated, state.sequence, &mut state.frame)
    {
        Ok(()) => respond_acquire_payload(request, config, &generated, state),
        Err(error) => respond_text(request, 500, &error.to_string()),
    }
    state.sequence += 1;
    state.schedule_next_frame(config);
}

fn require_simulated_camera(request: Request, camera: Option<String>) -> Option<Request> {
    if camera.as_deref() == Some(SIMULATED_CAMERA) {
        Some(request)
    } else {
        respond_text(request, 404, "unknown camera");
        None
    }
}

fn respond_acquire_payload(
    request: Request,
    config: &SimulatorConfig,
    generated: &SimulationSettings,
    state: &ServeState,
) {
    if inject_on_count(config.faults.malformed_payload_every, state.acquire_count) {
        respond_raw_json(request, b"{\"HoloLive Simulator\":{\"width\":1".to_vec());
        return;
    }

    let packed = match PackedArray::encode(
        &state.frame,
        generated.width,
        generated.height,
        config.wire_type,
        config.endianness,
    ) {
        Ok(packed) => packed,
        Err(_) => {
            respond_text(request, 500, "encode failed");
            return;
        }
    };
    respond_json(request, &BTreeMap::from([(SIMULATED_CAMERA, packed)]));
}

fn camera_from_param_path(path: &str) -> Option<String> {
    let encoded = path.strip_prefix(CAMERA_PARAM_PREFIX)?;
    urlencoding::decode(encoded)
        .ok()
        .map(|name| name.into_owned())
}

fn detector_from_acquire_url(url: &str) -> Option<String> {
    let query = url.split_once('?')?.1;
    query.split('&').find_map(|pair| {
        let (key, value) = pair.split_once('=')?;
        if key != "detectors" {
            return None;
        }
        urlencoding::decode(&value.replace('+', " "))
            .ok()
            .map(|name| name.into_owned())
    })
}

fn sleep_cancellable(cancelled: &AtomicBool, duration: Duration) -> bool {
    let Some(deadline) = Instant::now().checked_add(duration) else {
        return false;
    };
    while !cancelled.load(Ordering::Acquire) && Instant::now() < deadline {
        thread::sleep(
            Duration::from_millis(20).min(deadline.saturating_duration_since(Instant::now())),
        );
    }
    !cancelled.load(Ordering::Acquire)
}

fn respond_cameras(request: Request, config: &SimulatorConfig) {
    let camera = CameraInfo {
        detector_type: "CAMERA".into(),
        width: config.simulation.width,
        height: config.simulation.height,
        binnings: supported_binnings(config.simulation.width, config.simulation.height),
        extra: BTreeMap::from([("pixel_size(um)".into(), json!([1.0, 1.0]))]),
    };
    respond_json(request, &BTreeMap::from([(SIMULATED_CAMERA, camera)]));
}

fn camera_parameters(config: &SimulatorConfig) -> CameraParameters {
    let mut params = CameraParameters::acquisition(config.exposure_seconds, config.binning);
    params.0.extend([
        ("correction".into(), Value::from("UNPROCESSED")),
        ("exposure_mode".into(), Value::from("NONE")),
        ("image_size".into(), Value::from("FULL")),
        ("pre_exposure(s)".into(), Value::from(0.0)),
        ("pre_exposure_pause(s)".into(), Value::from(0.0)),
        ("shutter_mode".into(), Value::from("PRE_SPECIMEN")),
    ]);
    params
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CameraUpdate {
    binning: Option<u32>,
    #[serde(rename = "exposure(s)")]
    exposure_seconds: Option<f64>,
    #[serde(rename = "correction")]
    _correction: Option<String>,
    #[serde(rename = "exposure_mode")]
    _exposure_mode: Option<String>,
    #[serde(rename = "image_size")]
    _image_size: Option<String>,
    #[serde(rename = "pre_exposure(s)")]
    _pre_exposure_seconds: Option<f64>,
    #[serde(rename = "pre_exposure_pause(s)")]
    _pre_exposure_pause_seconds: Option<f64>,
    #[serde(rename = "shutter_mode")]
    _shutter_mode: Option<String>,
}

fn set_camera_parameters(mut request: Request, config: &mut SimulatorConfig) {
    let mut body = Vec::new();
    if request
        .as_reader()
        .take(4_097)
        .read_to_end(&mut body)
        .is_err()
        || body.len() > 4_096
    {
        respond_text(request, 400, "invalid camera parameter body");
        return;
    }
    let Ok(parameters) = serde_json::from_slice::<CameraUpdate>(&body) else {
        respond_text(request, 400, "invalid camera parameter JSON");
        return;
    };
    let mut next = config.clone();
    if let Some(binning) = parameters.binning {
        next.binning = binning;
    }
    if let Some(exposure) = parameters.exposure_seconds {
        next.exposure_seconds = exposure;
    }
    if next.validate().is_err() {
        respond_text(request, 400, "invalid exposure or binning");
        return;
    }
    *config = next;
    let _ = request.respond(Response::empty(StatusCode(204)));
}

fn supported_binnings(width: usize, height: usize) -> Vec<u32> {
    crate::simulator_config::SUPPORTED_BINNINGS
        .into_iter()
        .filter(|&binning| binning_divides(width, height, binning))
        .collect()
}

fn respond_json(request: Request, value: &impl Serialize) {
    match serde_json::to_vec(value) {
        Ok(body) => respond_raw_json(request, body),
        Err(error) => respond_text(request, 500, &error.to_string()),
    }
}

fn respond_raw_json(request: Request, body: Vec<u8>) {
    let accepts_gzip = request.headers().iter().any(|header| {
        header.field.equiv("Accept-Encoding")
            && header
                .value
                .as_str()
                .split(',')
                .any(|value| value.trim().starts_with("gzip"))
    });
    let mut response = if accepts_gzip && body.len() > 256 {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::new(5));
        match encoder.write_all(&body).and_then(|()| encoder.finish()) {
            Ok(compressed) => {
                Response::from_data(compressed).with_header(header("Content-Encoding", "gzip"))
            }
            Err(_) => Response::from_data(body),
        }
    } else {
        Response::from_data(body)
    };
    response.add_header(header("Content-Type", "application/json"));
    let _ = request.respond(response);
}

fn respond_text(request: Request, status: u16, message: &str) {
    let response = Response::from_string(message).with_status_code(StatusCode(status));
    let _ = request.respond(response);
}

fn header(name: &str, value: &str) -> Header {
    Header::from_bytes(name, value).expect("static HTTP header is valid")
}
