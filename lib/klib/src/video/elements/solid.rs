use vello::{
  kurbo::{Affine, Shape, Vec2},
  peniko::{Brush, Fill},
};

use crate::{style::Color32, timecode::Timecode, video::elements::VideoElement, Rect};

pub struct SolidVideoElement {
  color: Color32,
  rect: Rect,
  start_time: Timecode,
  end_time: Timecode,
}

impl SolidVideoElement {
  pub fn new(rect: Rect, color: Color32, start: Timecode, end: Timecode) -> SolidVideoElement {
    SolidVideoElement {
      rect,
      color,
      start_time: start,
      end_time: end,
    }
  }
}

impl VideoElement for SolidVideoElement {
  fn start_time(&self) -> Timecode {
    self.start_time
  }

  fn end_time(&self) -> Timecode {
    self.end_time
  }

  fn render(&self, scene: &mut vello::Scene, time: Timecode) {
    let kurbo_rect: vello::kurbo::Rect = self.rect.into();
    scene.fill(
      Fill::EvenOdd,
      Affine::IDENTITY,
      &Brush::Solid(self.color.into()),
      None,
      &kurbo_rect,
    );
  }
}
