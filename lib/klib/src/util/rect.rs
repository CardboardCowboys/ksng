use serde::{Deserialize, Serialize};

use crate::Point;

#[derive(Default, Debug, Serialize, Deserialize, Clone, Copy)]
pub struct Rect {
  pub x0: f32,
  pub y0: f32,
  pub x1: f32,
  pub y1: f32,
}

impl Rect {
  /// Returns the width of this rect (`x1 - x0`)
  pub fn width(&self) -> f32 {
    self.x1 - self.x0
  }

  /// Returns the height of this rect (`y1 - y0`)
  pub fn height(&self) -> f32 {
    self.y1 - self.y0
  }

  /// Returns the center of this rect.
  pub fn center(&self) -> Point {
    Point {
      x: self.x0 + (self.x1 - self.x0) / 2.0,
      y: self.y0 + (self.y1 - self.y0) / 2.0,
    }
  }

  /// Checks if this rect intersects with `rhs`.
  pub fn intersects(&self, rhs: &Rect) -> bool {
    self.x0 < rhs.x1 && self.x1 > rhs.x0 && self.y0 < rhs.y1 && self.y1 > rhs.y0
  }
}

impl From<Rect> for skia_safe::Rect {
  fn from(value: Rect) -> Self {
    skia_safe::Rect {
      left: value.x0,
      right: value.x1,
      top: value.y0,
      bottom: value.y1,
    }
  }
}

impl From<skia_safe::Rect> for Rect {
  fn from(value: skia_safe::Rect) -> Self {
    Rect {
      x0: value.left,
      x1: value.right,
      y0: value.top,
      y1: value.bottom,
    }
  }
}

#[derive(Default)]
pub struct RectBuilder {
  rect: Option<Rect>,
}

impl RectBuilder {
  pub fn add_point(&mut self, point: Point) {
    match &mut self.rect {
      None => {
        self.rect = Some(Rect {
          x0: point.x,
          y0: point.y,
          x1: point.x,
          y1: point.y,
        });
      }
      Some(rect) => {
        rect.x0 = rect.x0.min(point.x);
        rect.y0 = rect.y0.min(point.y);
        rect.x1 = rect.x1.max(point.x);
        rect.y1 = rect.y1.max(point.y);
      }
    }
  }

  pub fn add_rect(&mut self, rect: Rect) {
    self.add_point(Point {
      x: rect.x0,
      y: rect.y0,
    });
    self.add_point(Point {
      x: rect.x1,
      y: rect.y1,
    });
  }

  pub fn to_rect(&self) -> Option<Rect> {
    self.rect
  }
}
