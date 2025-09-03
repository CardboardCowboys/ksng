use skia_safe::Matrix;

use crate::{
  objects::event::Event, style::LyricsTrackStyle, timecode::Timecode,
  video::elements::VideoElement, Point, Rect,
};

struct TextMetrics {
  ascent: f32,
  descent: f32,
}

pub struct TextVideoElement {
  mat: Matrix,
  start: Timecode,
  end: Timecode,
  pos: Point,
  text: String,
  metrics: TextMetrics,
  blob: skia_safe::TextBlob,
  normal_paint: skia_safe::Paint,
  highlight_paint: skia_safe::Paint,
}

impl TextVideoElement {
  pub fn from_event(
    event: &Event,
    pos: Point,
    text: &str,
    blob: skia_safe::TextBlob,
    font: &skia_safe::Font,
    style: &LyricsTrackStyle,
  ) -> Box<dyn VideoElement> {
    let normal_color: skia_safe::Color4f = style.colors.normal.into();
    let highlight_color: skia_safe::Color4f = style.colors.highlight.into();
    let (_, metrics) = font.metrics();
    Box::new(TextVideoElement {
      mat: Matrix::new_identity(),
      start: event.start_timecode,
      end: event.end_timecode,
      pos,
      text: text.to_string(),
      metrics: TextMetrics {
        ascent: metrics.ascent,
        descent: metrics.descent,
      },
      blob,
      normal_paint: skia_safe::Paint::new(normal_color, None),
      highlight_paint: skia_safe::Paint::new(highlight_color, None),
    })
  }
}

impl VideoElement for TextVideoElement {
  fn start_time(&self) -> Timecode {
    self.start
  }

  fn end_time(&self) -> Timecode {
    self.end
  }

  fn bounds(&self) -> Rect {
    Rect {
      x0: self.pos.x,
      y0: self.pos.y + self.metrics.ascent,
      x1: self.pos.x + self.blob.bounds().width(),
      y1: self.pos.y + self.blob.bounds().height() + self.metrics.ascent + self.metrics.descent,
    }
  }

  fn transform(&self) -> Matrix {
    self.mat
  }

  fn set_transform(&mut self, mat: Matrix) {
    self.mat = mat;
  }

  fn render(&self, canvas: &skia_safe::Canvas, time: Timecode) {
    canvas.save();
    canvas.concat(&self.mat);
    let normalized_pos =
      ((time - self.start).0 as f32 / (self.end - self.start).0 as f32).clamp(0.0, 1.0);
    let draw_normal = normalized_pos < 1.0;
    let draw_highlight = normalized_pos > 0.0;

    let pos = Point {
      x: self.pos.x,
      y: self.pos.y + self.blob.bounds().height() + self.metrics.ascent + self.metrics.descent,
    };

    if draw_normal && draw_highlight {
      canvas.save();
      canvas.clip_rect(
        skia_safe::Rect {
          left: self.pos.x + self.blob.bounds().width() * normalized_pos,
          top: self.pos.y,
          right: self.pos.x + self.blob.bounds().width(),
          bottom: self.pos.y + self.blob.bounds().height(),
        },
        None,
        true,
      );

      canvas.draw_text_blob(self.blob.clone(), pos, &self.normal_paint);

      canvas.restore();
      canvas.save();

      canvas.clip_rect(
        skia_safe::Rect {
          left: self.pos.x,
          top: self.pos.y,
          right: self.pos.x + self.blob.bounds().width() * normalized_pos,
          bottom: self.pos.y + self.blob.bounds().height(),
        },
        None,
        true,
      );

      canvas.draw_text_blob(self.blob.clone(), pos, &self.highlight_paint);

      canvas.restore();
    } else if draw_normal {
      canvas.draw_text_blob(self.blob.clone(), pos, &self.normal_paint);
    } else if draw_highlight {
      canvas.draw_text_blob(self.blob.clone(), pos, &self.highlight_paint);
    }

    canvas.restore();
  }
}
