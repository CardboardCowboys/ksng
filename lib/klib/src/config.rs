use serde::{Deserialize, Serialize};

use crate::video::VideoConfig;

#[derive(Serialize, Deserialize, Default)]
pub struct Config {
  #[serde(default)]
  pub video: VideoConfig,
}
