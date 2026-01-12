use std::{
  fmt::{Display, Write},
  hash::Hash,
};

use klib::{
  objects::event::{Event, EventType, EventValue},
  timecode::Timecode,
};
use uuid::Uuid;

use crate::components::lyrics_editor::psuedo_event::PsuedoEvent;

pub struct LyricsEditorTextElement {
  pub event_type: EventType,
  pub start_timecode: Timecode,
  pub end_timecode: Timecode,
  pub events: Vec<PsuedoEvent>,
}

impl LyricsEditorTextElement {
  pub fn new(event_type: EventType, events: Vec<Event>) -> Self {
    let start_timecode = events
      .iter()
      .map(|e| e.start_timecode)
      .min()
      .unwrap_or_default();
    let end_timecode = events
      .iter()
      .map(|e| e.end_timecode)
      .max()
      .unwrap_or_default();
    LyricsEditorTextElement {
      event_type,
      start_timecode,
      end_timecode,
      events: events.into_iter().map(PsuedoEvent::from_event).collect(),
    }
  }
}

impl Display for LyricsEditorTextElement {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self.event_type {
      EventType::LineBreak => f.write_char('\n')?,
      EventType::ParagraphBreak => f.write_str("\n\n")?,
      EventType::Lyric => {
        let mut first = true;
        for e in &self.events {
          let Some(text) = &e.value else {
            continue;
          };

          if !first {
            f.write_char(super::SYLLABLE_SEPARATOR)?;
          } else {
            first = false;
          }

          for ch in text.chars() {
            if ch == super::SYLLABLE_SEPARATOR {
              f.write_char(super::ESCAPE_CHAR)?;
            }

            f.write_char(ch)?;
          }
        }
      }
      _ => {}
    }

    Ok(())
  }
}
