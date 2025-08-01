use parley::{GlyphRun, Layout, PositionedLayoutItem, Rect};
use vello::{
  kurbo::{Affine, Point, Shape},
  peniko::{Brush, Fill, Mix},
  Scene,
};

use crate::{
  objects::event::Event, style::LyricsTrackStyle, timecode::Timecode, video::elements::VideoElement,
};

pub struct TextVideoElement {
  start: Timecode,
  end: Timecode,
  pos: Point,
  text: String,
  layout: Layout<[u8; 4]>,
  normal_brush: Brush,
  highlight_brush: Brush,
}

impl TextVideoElement {
  pub fn from_event(
    event: &Event,
    pos: Point,
    text: &str,
    layout: Layout<[u8; 4]>,
    style: &LyricsTrackStyle,
  ) -> Box<dyn VideoElement> {
    Box::new(TextVideoElement {
      start: event.start_timecode,
      end: event.end_timecode,
      pos,
      text: text.to_string(),
      layout,
      normal_brush: Brush::Solid(style.colors.normal.into()),
      highlight_brush: Brush::Solid(style.colors.highlight.into()),
    })
  }

  fn draw_glyph_run(
    &self,
    scene: &mut Scene,
    glyph_run: &GlyphRun<'_, [u8; 4]>,
    brush: &Brush,
    transform: Affine,
  ) {
    let mut x = glyph_run.offset();
    let y = glyph_run.baseline();
    let run = glyph_run.run();
    let font = run.font();
    let font_size = run.font_size();
    let synthesis = run.synthesis();
    let glyph_transform = synthesis
      .skew()
      .map(|angle| Affine::skew(angle.to_radians().tan() as f64, 0.0));

    scene
      .draw_glyphs(font)
      .brush(brush)
      .hint(true)
      .transform(transform)
      .glyph_transform(glyph_transform)
      .font_size(font_size)
      .normalized_coords(run.normalized_coords())
      .draw(
        Fill::NonZero,
        glyph_run.glyphs().map(|glyph| {
          let gx = x + glyph.x;
          let gy = y - glyph.y;
          x += glyph.advance;
          vello::Glyph {
            id: glyph.id as _,
            x: gx,
            y: gy,
          }
        }),
      )
  }
}

impl VideoElement for TextVideoElement {
  fn start_time(&self) -> Timecode {
    self.start
  }

  fn end_time(&self) -> Timecode {
    self.end
  }

  fn render(&self, scene: &mut vello::Scene, time: Timecode) {
    let transform = Affine::translate((self.pos.x, self.pos.y));

    let normalized_pos = (self.end - time).0 as f32 / (self.end - self.start).0 as f32;
    let draw_normal = normalized_pos < 1.0;
    let draw_highlight = normalized_pos > 0.0;

    for line in self.layout.lines() {
      for item in line.items() {
        let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
          continue;
        };

        if draw_normal && draw_highlight {
          scene.push_layer(
            Mix::Clip,
            1.0,
            Affine::IDENTITY,
            &Rect {
              x0: self.pos.x,
              y0: self.pos.y,
              x1: self.pos.x + (glyph_run.advance() * normalized_pos) as f64,
              y1: self.pos.y + self.layout.height() as f64,
            },
          );

          self.draw_glyph_run(scene, &glyph_run, &self.normal_brush, transform);

          scene.pop_layer();
          scene.push_layer(
            Mix::Clip,
            1.0,
            Affine::IDENTITY,
            &Rect {
              x0: self.pos.x,
              y0: self.pos.y,
              x1: self.pos.x + (glyph_run.advance() * (1.0 - normalized_pos)) as f64,
              y1: self.pos.y + self.layout.height() as f64,
            },
          );

          self.draw_glyph_run(scene, &glyph_run, &self.highlight_brush, transform);

          scene.pop_layer();
        } else if draw_normal {
          self.draw_glyph_run(scene, &glyph_run, &self.normal_brush, transform);
        } else if draw_highlight {
          self.draw_glyph_run(scene, &glyph_run, &self.highlight_brush, transform);
        }
      }
    }
  }
}
