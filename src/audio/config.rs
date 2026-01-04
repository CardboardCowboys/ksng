use std::str::FromStr;

use cpal::{
  traits::{DeviceTrait, HostTrait},
  Device, DeviceId, HostId,
};
use serde::{Deserialize, Serialize};
use serde_json::Map;

#[derive(Serialize, Deserialize, Clone)]
pub struct AudioConfig {
  pub host: String,
  pub device: Option<String>,
}

impl Default for AudioConfig {
  fn default() -> Self {
    let host = cpal::default_host();
    let device = host
      .default_output_device()
      .and_then(|d| d.id().ok())
      .map(|d| d.1);
    Self {
      host: host.id().name().to_string(),
      device,
    }
  }
}

impl AudioConfig {
  pub fn hosts() -> Vec<String> {
    cpal::available_hosts()
      .iter()
      .map(|h| h.name().to_string())
      .collect()
  }

  pub fn devices(host: HostId) -> Vec<Device> {
    cpal::host_from_id(host)
      .ok()
      .and_then(|d| d.output_devices().ok())
      .map(|d| d.collect())
      .unwrap_or_default()
  }

  pub fn device_name(host: &str, device_id: &str) -> Option<String> {
    let host_id = HostId::from_str(host).ok()?;
    let device_id = DeviceId(host_id, device_id.to_string());
    cpal::host_from_id(host_id)
      .ok()
      .and_then(|h| h.device_by_id(&device_id))
      .and_then(|d| d.description().ok())
      .map(|d| format!("{} ({})", d.name(), d.driver().unwrap_or("")))
  }
}
