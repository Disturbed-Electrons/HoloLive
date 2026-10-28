use std::{collections::BTreeMap, io::Read, time::Duration};

use reqwest::{StatusCode, blocking::Response};
use serde::de::DeserializeOwned;

use crate::{
    DecodedArray, PackedArray, ProtocolError,
    protocol::{
        ACQUIRE_PATH, CAMERAS_PATH, CameraInfo, CameraParameters, VERSION_PATH, camera_param_path,
    },
};

const MAX_JSON_RESPONSE_BYTES: usize = 64 * 1024 * 1024;
const MAX_ERROR_MESSAGE_BYTES: usize = 64 * 1024;
const MAX_ACQUIRE_RESPONSE_BYTES: usize =
    (crate::array::MAX_IMAGE_PIXELS * crate::ScalarType::FLOAT64.bytes()).div_ceil(3) * 4
        + 64 * 1024;

#[derive(Clone)]
pub struct TemscriptClient {
    base_url: reqwest::Url,
    http: reqwest::blocking::Client,
}

impl TemscriptClient {
    pub fn new(base_url: impl Into<String>) -> Result<Self, ProtocolError> {
        let base_url = format!("{}/", base_url.into().trim_end_matches('/'));
        let base_url = reqwest::Url::parse(&base_url)
            .map_err(|error| ProtocolError::InvalidServerUrl(error.to_string()))?;
        if !matches!(base_url.scheme(), "http" | "https")
            || base_url.query().is_some()
            || base_url.fragment().is_some()
        {
            return Err(ProtocolError::InvalidServerUrl(base_url.to_string()));
        }
        let http = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(3))
            .timeout(Duration::from_secs(15))
            .no_proxy()
            .gzip(true)
            .build()?;
        Ok(Self { base_url, http })
    }

    pub fn version(&self) -> Result<String, ProtocolError> {
        self.get_json(VERSION_PATH)
    }

    pub fn cameras(&self) -> Result<BTreeMap<String, CameraInfo>, ProtocolError> {
        self.get_json(CAMERAS_PATH)
    }

    pub fn camera_info(&self, camera: &str) -> Result<CameraInfo, ProtocolError> {
        self.cameras()?
            .remove(camera)
            .ok_or_else(|| ProtocolError::MissingCamera(camera.to_owned()))
    }

    pub fn set_camera_parameters(
        &self,
        camera: &str,
        parameters: &CameraParameters,
    ) -> Result<(), ProtocolError> {
        self.set_camera_parameters_with_timeout(camera, parameters, Duration::from_secs(15))
    }

    pub fn set_camera_parameters_with_timeout(
        &self,
        camera: &str,
        parameters: &CameraParameters,
        timeout: Duration,
    ) -> Result<(), ProtocolError> {
        let endpoint = camera_param_path(camera);
        let response = self
            .http
            .put(self.url(&endpoint))
            .timeout(timeout)
            .json(parameters)
            .send()?;
        accept_empty(response, &endpoint)
    }

    pub fn acquire(&self, camera: &str) -> Result<DecodedArray, ProtocolError> {
        let endpoint = ACQUIRE_PATH;
        let response = self
            .http
            .get(self.url(endpoint))
            .query(&[("detectors", camera)])
            .header(reqwest::header::ACCEPT, "application/json")
            .send()?;
        let arrays: BTreeMap<String, PackedArray> =
            decode_response(response, endpoint, MAX_ACQUIRE_RESPONSE_BYTES)?;
        arrays
            .get(camera)
            .ok_or_else(|| ProtocolError::MissingDetector(camera.to_owned()))?
            .decode()
    }

    fn get_json<T: DeserializeOwned>(&self, endpoint: &str) -> Result<T, ProtocolError> {
        decode_response(
            self.http
                .get(self.url(endpoint))
                .header(reqwest::header::ACCEPT, "application/json")
                .send()?,
            endpoint,
            MAX_JSON_RESPONSE_BYTES,
        )
    }

    fn url(&self, endpoint: &str) -> reqwest::Url {
        self.base_url
            .join(endpoint.trim_start_matches('/'))
            .expect("temscript endpoints are relative URLs")
    }
}

fn http_error(response: Response, endpoint: &str) -> ProtocolError {
    let status = response.status().as_u16();
    let message = read_limited(response, MAX_ERROR_MESSAGE_BYTES)
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .unwrap_or_else(|error| error.to_string());
    ProtocolError::Http {
        endpoint: endpoint.into(),
        status,
        message,
    }
}

fn decode_response<T: DeserializeOwned>(
    response: Response,
    endpoint: &str,
    limit_bytes: usize,
) -> Result<T, ProtocolError> {
    let status = response.status();
    if status != StatusCode::OK {
        return Err(http_error(response, endpoint));
    }
    let bytes = read_limited(response, limit_bytes)?;
    Ok(serde_json::from_slice(&bytes)?)
}

fn read_limited(mut reader: impl Read, limit_bytes: usize) -> Result<Vec<u8>, ProtocolError> {
    let mut bytes = Vec::new();
    reader
        .by_ref()
        .take(limit_bytes as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > limit_bytes {
        return Err(ProtocolError::ResponseTooLarge { limit_bytes });
    }
    Ok(bytes)
}

fn accept_empty(response: Response, endpoint: &str) -> Result<(), ProtocolError> {
    if matches!(response.status(), StatusCode::OK | StatusCode::NO_CONTENT) {
        Ok(())
    } else {
        Err(http_error(response, endpoint))
    }
}
