use serde::{Deserialize, Serialize};

pub mod config;
pub mod error;
pub mod objects;
pub mod style;
pub mod timecode;
pub mod util;
pub mod video;

pub use util::rect::Rect;

#[derive(Serialize, Deserialize, Clone, Copy)]
pub struct Point {
  pub x: f32,
  pub y: f32,
}

impl From<Point> for skia_safe::Point {
  fn from(value: Point) -> Self {
    skia_safe::Point {
      x: value.x,
      y: value.y,
    }
  }
}
