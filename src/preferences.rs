use serde::{Deserialize, Serialize};

use crate::audio::config::AudioConfig;

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct Preferences {
  pub audio_config: AudioConfig,
}
