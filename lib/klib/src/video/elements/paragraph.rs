use skia_safe::Matrix;

use crate::{timecode::Timecode, util::rect::RectBuilder, video::elements::VideoElement, Rect};

pub struct ParagraphVideoElement {
  mat: Matrix,
  elements: Vec<Box<dyn VideoElement>>,
  start_time: Timecode,
  end_time: Timecode,
}

impl ParagraphVideoElement {
  pub fn from_elements<T: Iterator<Item = Box<dyn VideoElement>>>(
    elements: T,
  ) -> ParagraphVideoElement {
    let mut elements: Vec<Box<dyn VideoElement>> = elements.collect();
    elements.sort_by_key(|a| a.start_time());
    let start_time = elements
      .iter()
      .map(|e| e.start_time())
      .min()
      .unwrap_or(Timecode(0));
    let end_time = elements
      .iter()
      .map(|e| e.end_time())
      .max()
      .unwrap_or(Timecode(0));

    ParagraphVideoElement {
      mat: Matrix::new_identity(),
      elements,
      start_time,
      end_time,
    }
  }
}

impl VideoElement for ParagraphVideoElement {
  fn start_time(&self) -> Timecode {
    self.start_time
  }

  fn end_time(&self) -> Timecode {
    self.end_time
  }

  fn transform(&self) -> Matrix {
    self.mat
  }

  fn set_transform(&mut self, mat: Matrix) {
    self.mat = mat;
  }

  fn bounds(&self) -> Rect {
    if self.elements.is_empty() {
      return Rect::default();
    }

    let mut builder = RectBuilder::new();
    for e in &self.elements {
      builder.add_rect(e.bounds());
    }

    builder.to_rect().unwrap_or_default()
  }

  fn render(&self, canvas: &skia_safe::Canvas, time: Timecode) {
    canvas.save();
    canvas.concat(&self.mat);
    for element in &self.elements {
      element.render(canvas, time);
    }
    canvas.restore();
  }
}
