use std::{
    panic,
    sync::{Arc, Mutex, Once},
    time::Duration,
};

use temscript_client::{CameraParameters, TemscriptClient};
use tracing::{debug, info};

use crate::config::{closest_binning, supported_binnings};

use super::status::RuntimeStatus;

struct PanicReset {
    client: TemscriptClient,
    camera: String,
}

static PANIC_RESET: Mutex<Option<PanicReset>> = Mutex::new(None);

pub(crate) fn install_panic_reset_hook() {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        let previous = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            reset_armed_camera();
            previous(info);
        }));
    });
}

pub(crate) fn arm_panic_reset(client: TemscriptClient, camera: String) {
    if let Ok(mut slot) = PANIC_RESET.lock() {
        *slot = Some(PanicReset { client, camera });
    }
}

pub(crate) fn disarm_panic_reset() {
    if let Ok(mut slot) = PANIC_RESET.lock() {
        *slot = None;
    }
}

const RESET_EXPOSURE_SECONDS: f64 = 4.0;
const RESET_BINNING: u32 = 1;

fn reset_armed_camera() {
    let Some(PanicReset { client, camera }) =
        PANIC_RESET.lock().ok().and_then(|mut slot| slot.take())
    else {
        return;
    };
    let _ = configure_camera(
        &client,
        &camera,
        RESET_EXPOSURE_SECONDS,
        RESET_BINNING,
        Some(Duration::from_secs(1)),
    );
}

pub(crate) fn configure_camera(
    client: &TemscriptClient,
    camera: &str,
    exposure_seconds: f64,
    binning: u32,
    timeout: Option<Duration>,
) -> Result<(), temscript_client::ProtocolError> {
    let put = |parameters: &CameraParameters| match timeout {
        Some(timeout) => client.set_camera_parameters_with_timeout(camera, parameters, timeout),
        None => client.set_camera_parameters(camera, parameters),
    };
    put(&CameraParameters::acquisition(exposure_seconds, binning))?;
    // Titan 1.1 / Gatan US1000: setting binning overwrites exposure.
    put(&CameraParameters::exposure(exposure_seconds))?;
    Ok(())
}

pub(crate) fn reset_camera(
    client: &TemscriptClient,
    configured_camera: &Arc<Mutex<Option<String>>>,
    timeout: Option<Duration>,
) {
    let Some(camera) = configured_camera
        .lock()
        .expect("configured-camera lock poisoned")
        .take()
    else {
        return;
    };
    let result = configure_camera(
        client,
        &camera,
        RESET_EXPOSURE_SECONDS,
        RESET_BINNING,
        timeout,
    );
    if let Err(error) = result {
        debug!(%error, camera, "camera reset failed");
    } else {
        info!(
            camera,
            exposure_seconds = RESET_EXPOSURE_SECONDS,
            binning = RESET_BINNING,
            "reset camera settings"
        );
    }
}

pub(crate) fn initialize_camera(
    client: &TemscriptClient,
    camera: &str,
    exposure_seconds: f64,
    binning: u32,
    status: &Mutex<RuntimeStatus>,
    configured_camera: &Mutex<Option<String>>,
) -> Result<u32, String> {
    client.version().map_err(|error| error.to_string())?;
    let info = client
        .camera_info(camera)
        .map_err(|error| error.to_string())?;
    let binnings = supported_binnings(&info);
    let selected = closest_binning(&binnings, binning)
        .ok_or_else(|| format!("camera {camera:?} advertises no binning modes"))?;
    *configured_camera
        .lock()
        .expect("configured-camera lock poisoned") = Some(camera.to_owned());
    configure_camera(client, camera, exposure_seconds, selected, None)
        .map_err(|error| error.to_string())?;
    let mut status = status.lock().expect("status lock poisoned");
    status.supported_binnings = binnings;
    status.mark_connected();
    Ok(selected)
}
