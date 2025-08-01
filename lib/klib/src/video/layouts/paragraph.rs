use parley::Layout;
use serde::{Deserialize, Serialize};
use vello::kurbo::Point;

use crate::{
  objects::{
    event::{Event, EventType},
    track::{Track, TrackType},
  },
  video::{
    context::LyricsTrackContext,
    elements::{paragraph::ParagraphVideoElement, text::TextVideoElement, VideoElement},
  },
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

    let events: Vec<&Event> = track.events.iter().collect();
    let mut elements: Vec<Box<dyn VideoElement>> = Vec::new();

    let mut next_index = 0;
    while next_index < events.len() {
      let mut page_elements = Vec::new();
      next_index = Self::fill_paragraph(context, &events, next_index, &mut page_elements);
      match self.merger_mode {
        ParagraphMergerMode::None => {
          for (_, elem) in page_elements {
            elements.push(Box::new(ParagraphVideoElement::from_elements(
              [elem].into_iter(),
            )));
          }
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
    events: &[&Event],
    start_index: usize,
    out_elements: &mut Vec<(usize, Box<dyn VideoElement>)>,
  ) -> usize {
    let rect = context.area;
    let (space_width, _) = Self::measure_text(context, " ");

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
          let layout = Self::build_layout(context, text);
          let (width, height) = (layout.width() as f64, layout.height() as f64);
          let x = if first_on_line {
            next_x
          } else {
            next_x + space_width
          };

          if (x + width) > rect.x1 {
            // Too large, new line
            next_x = rect.x0;
            next_y = line_y;
            first_on_line = true;
            line_num += 1;
          } else {
            first_on_line = false;
            next_x = x + width;
          }

          if (next_y + height) > rect.y1 {
            // Paragraph done
            break;
          }

          line_y = line_y.max(next_y + height);

          out_elements.push((
            line_num,
            TextVideoElement::from_event(
              event,
              Point {
                x: next_x,
                y: next_y,
              },
              text,
              layout,
              context.style,
            ),
          ));
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

  fn build_layout<'a>(context: &mut LyricsTrackContext<'a>, text: &str) -> Layout<[u8; 4]> {
    let mut builder =
      context
        .layout_context
        .ranged_builder(&mut context.font_context, text, 1.0, false);
    context.style.font.push_builder(&mut builder);
    builder.build(text)
  }

  fn measure_text<'a>(context: &mut LyricsTrackContext<'a>, text: &str) -> (f64, f64) {
    let layout = Self::build_layout(context, text);
    (layout.width() as f64, layout.height() as f64)
  }
}
