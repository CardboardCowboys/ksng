use serde::{Deserialize, Serialize};

pub mod config;
pub mod error;
pub mod objects;
pub mod style;
pub mod timecode;
pub mod video;

#[derive(Serialize, Deserialize, Clone, Copy)]
pub struct Rect {
  pub x0: f64,
  pub y0: f64,
  pub x1: f64,
  pub y1: f64,
}

impl From<Rect> for parley::Rect {
  fn from(value: Rect) -> Self {
    parley::Rect {
      x0: value.x0,
      x1: value.x1,
      y0: value.y0,
      y1: value.y1,
    }
  }
}

impl From<parley::Rect> for Rect {
  fn from(value: parley::Rect) -> Self {
    Rect {
      x0: value.x0,
      x1: value.x1,
      y0: value.y0,
      y1: value.y1,
    }
  }
}
