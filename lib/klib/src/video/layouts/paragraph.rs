use std::collections::HashMap;

use harfbuzz_rs::{GlyphBuffer, UnicodeBuffer};
use serde::{Deserialize, Serialize};
use skia_safe::{GlyphId, Matrix};

use crate::{
  objects::{
    event::{Event, EventType},
    track::{Track, TrackType},
  },
  util::{rect::RectBuilder, skfont_to_harfbuzz_font},
  video::{
    context::LyricsTrackContext,
    elements::{paragraph::ParagraphVideoElement, text::TextVideoElement, VideoElement},
  },
  Point, Rect,
};

/// How video elements are combined together to form paragraphs.
#[derive(Serialize, Deserialize, Clone, Copy)]
pub enum ParagraphMergerMode {
  /// Each lyric is its own element and is handled independently.
  None,
  /// Each line is its own element.
  Line,
  /// Each `n` lines form one element.
  MultiLine(usize),
  /// Each page of lines form one element.
  Page,
}

pub struct ParagraphLayout {
  merger_mode: ParagraphMergerMode,
}

struct LineWidthInfo {
  current_line: usize,
  current_start: f32,
  last_end: f32,
}

impl ParagraphLayout {
  pub fn new(merger_mode: ParagraphMergerMode) -> ParagraphLayout {
    ParagraphLayout { merger_mode }
  }

  pub fn layout_track<'a>(
    &self,
    context: &mut LyricsTrackContext<'a>,
    track: &Track,
  ) -> Vec<Box<dyn VideoElement>> {
    if track.track_type != TrackType::Lyrics {
      return vec![];
    }

    let Some(font) = context.style.font.to_skfont(&context.font_mgr) else {
      log::warn!(
        "Couldn't create font {:?}, skipping layout",
        context.style.font
      );
      return vec![];
    };

    let hbfont = skfont_to_harfbuzz_font(&font);

    let events: Vec<&Event> = track.events.iter().collect();
    let mut elements: Vec<Box<dyn VideoElement>> = Vec::new();

    let mut next_index = 0;
    while next_index < events.len() {
      let mut page_elements = Vec::new();
      next_index = Self::fill_paragraph(
        context,
        &font,
        &hbfont,
        &events,
        next_index,
        &mut page_elements,
      );

      // Centrist
      let mut page_rect_builder = RectBuilder::default();
      let mut line_height: f32 = 0.0;

      let mut line_widths = HashMap::new();
      let mut line_width_info = None;
      for (line, elem) in &page_elements {
        let rect = elem.bounds();
        page_rect_builder.add_rect(rect);
        line_height = line_height.max(rect.height());
        match &mut line_width_info {
          None => {
            line_width_info = Some(LineWidthInfo {
              current_line: *line,
              current_start: rect.x0,
              last_end: rect.x1,
            });
          }
          Some(info) => {
            if *line != info.current_line {
              line_widths.insert(info.current_line, info.last_end - info.current_start);
              line_width_info = Some(LineWidthInfo {
                current_line: *line,
                current_start: rect.x0,
                last_end: rect.x1,
              });
            } else {
              info.last_end = rect.x1;
            }
          }
        };
      }

      if let Some(info) = line_width_info {
        // handle remaining line width
        line_widths.insert(info.current_line, info.last_end - info.current_start);
      }

      let page_height = page_rect_builder.to_rect().unwrap_or_default().height();
      let y_offset =
        context.area.y0 + context.area.height() / 2.0 - page_height / 2.0 - line_height / 2.0;
      for (line, e) in &mut page_elements {
        let Some(width) = line_widths.get(line) else {
          continue;
        };

        e.set_transform(Matrix::translate(Point {
          x: context.area.x0 + context.area.width() / 2.0 - width / 2.0,
          y: y_offset,
        }));
      }

      match self.merger_mode {
        ParagraphMergerMode::None => {
          elements.extend(page_elements.into_iter().map(|(_, e)| e));
        }
        ParagraphMergerMode::Line => {
          let mut line_index = 0;
          let mut line_elements = Vec::new();
          for (line, elem) in page_elements {
            if line_index != line {
              if !line_elements.is_empty() {
                elements.push(Box::new(ParagraphVideoElement::from_elements(
                  line_elements.into_iter(),
                )));
                line_elements = Vec::new();
              }

              line_index = line;
            }

            line_elements.push(elem);
          }
        }
        ParagraphMergerMode::MultiLine(n) => {
          let mut num_lines_pending = 0;
          let mut line_index = 0;
          let mut line_elements = Vec::new();
          for (line, elem) in page_elements {
            if line_index != line {
              num_lines_pending += 1;
              if num_lines_pending >= n {
                elements.push(Box::new(ParagraphVideoElement::from_elements(
                  line_elements.into_iter(),
                )));

                line_elements = Vec::new();
              }

              line_index = line;
            }

            line_elements.push(elem);
          }
        }
        ParagraphMergerMode::Page => {
          elements.push(Box::new(ParagraphVideoElement::from_elements(
            page_elements.into_iter().map(|(_, e)| e),
          )));
        }
      }
    }

    elements
  }

  fn fill_paragraph<'a>(
    context: &mut LyricsTrackContext<'a>,
    font: &skia_safe::Font,
    hbfont: &harfbuzz_rs::Owned<harfbuzz_rs::Font<'_>>,
    events: &[&Event],
    start_index: usize,
    out_elements: &mut Vec<(usize, Box<dyn VideoElement>)>,
  ) -> usize {
    let rect = context.area;

    let (space_width, _) = font.measure_str(" ", None);

    let upem = hbfont.face().upem() as f64;
    let size = font.size() as f64;

    let mut char_positions = Vec::new();
    let mut glyphs = Vec::new();

    let mut idx = start_index;
    let mut line_num = 0;
    let mut first_on_line = true;
    let mut next_x = rect.x0;
    let mut line_y = rect.y0;
    let mut next_y = rect.y0;
    while idx < events.len() {
      let event = events[idx];
      idx += 1;

      match event.event_type {
        EventType::Lyric if event.text().is_some() => {
          let text = event.text().unwrap();
          let mut text_buffer = UnicodeBuffer::new().add_str(text);
          let mut shape = harfbuzz_rs::shape(hbfont, text_buffer, &[]);
          let layout_rect = Self::compute_rect_from_glyphs(font, hbfont, &shape);

          let (width, height) = (layout_rect.width(), layout_rect.height());
          let x = if first_on_line || event.linked_id.is_some() {
            next_x
          } else {
            next_x + space_width
          };

          if (x + width) > rect.x1 {
            // Too large, new line
            next_x = rect.x0;
            next_y = line_y;
            first_on_line = false;
            line_num += 1;
          } else {
            next_x = x;
            first_on_line = false;
          }

          if (next_y + height) > rect.y1 {
            // Paragraph done
            idx -= 1;
            break;
          }

          // Check if the next event is linked and will overflow, and if so hyphenate this one.
          // This is pretty inefficient but should be fine since it's happening once per layout gen... right?
          if idx < events.len() && events[idx].linked_id.is_some() {
            let next_text = events[idx].text().unwrap();
            let next_text_buffer = UnicodeBuffer::new().add_str(next_text);
            let next_shape = harfbuzz_rs::shape(hbfont, next_text_buffer, &[]);
            let next_layout_rect = Self::compute_rect_from_glyphs(font, hbfont, &next_shape);

            if next_x + width + next_layout_rect.width() > rect.x1 {
              let new_text = text.to_string() + "-";
              text_buffer = UnicodeBuffer::new().add_str(&new_text);
              shape = harfbuzz_rs::shape(hbfont, text_buffer, &[]);
            }
          }

          line_y = line_y.max(next_y + height * context.style.line_height_multiplier);

          char_positions.clear();
          glyphs.clear();

          let mut glyph_x = 0.0;
          for (pos, info) in shape
            .get_glyph_positions()
            .iter()
            .zip(shape.get_glyph_infos())
          {
            let x_advance = (pos.x_advance as f64 * size) / upem;
            let x_offset = (pos.x_offset as f64 * size) / upem;
            let y_offset = (pos.y_offset as f64 * size) / upem;
            glyphs.push(info.codepoint as GlyphId);
            char_positions.push(skia_safe::Point {
              x: (glyph_x + x_offset) as f32,
              y: y_offset as f32,
            });

            glyph_x += x_advance;
          }

          let Some(blob) =
            skia_safe::TextBlob::from_pos_text(glyphs.as_slice(), &char_positions, font)
          else {
            log::warn!("failed to create text blob for string {text}");
            return events.len();
          };

          out_elements.push((
            line_num,
            TextVideoElement::from_event(
              event,
              Point {
                x: next_x,
                y: next_y,
              },
              text,
              blob,
              font,
              context.style,
            ),
          ));

          next_x += width;
        }
        EventType::LineBreak => {
          // Forced new line
          next_x = rect.x0;
          next_y = line_y;
          first_on_line = true;
          line_num += 1;
        }
        EventType::ParagraphBreak => break,
        _ => continue,
      }
    }

    idx
  }

  fn compute_rect_from_glyphs(
    skfont: &skia_safe::Font,
    hbfont: &harfbuzz_rs::Font,
    buffer: &GlyphBuffer,
  ) -> Rect {
    let upem = hbfont.face().upem() as f64;
    let size = skfont.size() as f64;

    let mut builder = RectBuilder::default();
    let mut x = 0.0;
    for (pos, info) in buffer
      .get_glyph_positions()
      .iter()
      .zip(buffer.get_glyph_infos())
    {
      let glyph_v = hbfont.get_glyph_v_advance(info.codepoint);
      let x_advance = (pos.x_advance as f64 * size) / upem;
      let y_advance = (glyph_v as f64 * size) / upem;
      let x_offset = (pos.x_offset as f64 * size) / upem;
      let y_offset = (pos.y_offset as f64 * size) / upem;

      builder.add_point(Point {
        x: (x + x_offset) as f32,
        y: y_offset as f32,
      });
      builder.add_point(Point {
        x: (x + x_advance) as f32,
        y: y_advance as f32,
      });

      x += x_advance;
    }

    builder.to_rect().unwrap_or_default()
  }
}
