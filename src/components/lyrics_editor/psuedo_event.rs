use std::hash::Hash;

use klib::{
  objects::event::{Event, EventType, EventValue},
  timecode::Timecode,
};
use uuid::Uuid;

use crate::components::lyrics_editor::{BLOCK_CLOSE, BLOCK_OPEN, ESCAPE_CHAR, SYLLABLE_SEPARATOR};

/// A representation of an event that could be from a project or could be
/// synthetic to participate in the diffing process.
#[derive(Clone)]
pub struct PsuedoEvent {
  pub value: Option<String>,
  pub event_type: EventType,
  pub linked_to_prev: bool,
  pub start_timecode: Timecode,
  pub end_timecode: Timecode,
  pub id: Uuid,
}

impl Hash for PsuedoEvent {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.value.hash(state);
    self.event_type.hash(state);
  }
}

impl PartialEq for PsuedoEvent {
  fn eq(&self, other: &Self) -> bool {
    self.value == other.value && self.event_type == other.event_type
  }
}

impl Eq for PsuedoEvent {}

impl PartialOrd for PsuedoEvent {
  fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
    Some(self.cmp(other))
  }
}

impl Ord for PsuedoEvent {
  fn cmp(&self, other: &Self) -> std::cmp::Ordering {
    self.start_timecode.cmp(&other.start_timecode)
  }
}

impl PsuedoEvent {
  pub fn from_event(ev: Event) -> PsuedoEvent {
    PsuedoEvent {
      value: match ev.value {
        Some(EventValue::Lyric { text }) => Some(text),
        _ => None,
      },
      event_type: ev.event_type,
      id: ev.id,
      start_timecode: ev.start_timecode,
      end_timecode: ev.end_timecode,
      linked_to_prev: ev.linked_id.is_some(),
    }
  }

  pub fn to_event(&self, real_start: Timecode, real_end: Timecode) -> Event {
    Event {
      id: self.id,
      linked_id: None,
      start_timecode: real_start,
      end_timecode: real_end.max(real_start + Timecode(50)),
      event_type: self.event_type,
      value: self
        .value
        .as_ref()
        .map(|text| EventValue::Lyric { text: text.clone() }),
    }
  }

  fn synthetic_event(
    i: usize,
    event_type: EventType,
    text: Option<String>,
    linked: bool,
  ) -> PsuedoEvent {
    PsuedoEvent {
      value: text,
      event_type,
      linked_to_prev: linked,
      start_timecode: Timecode((i * 100) as u32),
      end_timecode: Timecode((i * 100) as u32 + 1),
      id: Uuid::new_v4(),
    }
  }

  pub fn from_str(s: &str) -> Vec<PsuedoEvent> {
    let mut events = Vec::new();
    let mut line_count = 0;
    let mut block_level = 0;
    let mut current_type = None;
    let mut current_value = String::new();
    let mut linked = false;
    let mut is_escaped = false;

    for c in s.chars() {
      if c == '\n' {
        if matches!(current_type, Some(EventType::Lyric)) {
          events.push(Self::synthetic_event(
            events.len(),
            EventType::Lyric,
            Some(current_value),
            linked,
          ));
          linked = false;
          current_value = String::new();
          current_type = None;
        }

        line_count += 1;
        if line_count > 1 {
          events.push(Self::synthetic_event(
            events.len(),
            EventType::ParagraphBreak,
            None,
            false,
          ));
          line_count = 0;
        }

        continue;
      }

      if c == '\r' {
        continue;
      }

      // not a line break or paragraph break, so push the last line break if we need
      if line_count > 0 {
        events.push(Self::synthetic_event(
          events.len(),
          EventType::LineBreak,
          None,
          false,
        ));
        line_count = 0;
      }

      if c.is_whitespace() {
        if block_level <= 0 {
          if matches!(current_type, Some(EventType::Lyric)) {
            events.push(Self::synthetic_event(
              events.len(),
              EventType::Lyric,
              Some(current_value),
              linked,
            ));
            linked = false;
            current_value = String::new();
          }
        } else {
          current_value.push(c);
        }

        continue;
      }

      if c == ESCAPE_CHAR && !is_escaped {
        is_escaped = true;
        continue;
      }

      if is_escaped {
        is_escaped = false;
        if c != ESCAPE_CHAR && c != SYLLABLE_SEPARATOR && c != BLOCK_OPEN && c != BLOCK_CLOSE {
          // if it's not escaping something that needs to be escaped, just include it
          current_value.push(ESCAPE_CHAR);
        }

        current_type = Some(EventType::Lyric);
        current_value.push(c);
        continue;
      }

      if c == SYLLABLE_SEPARATOR {
        if matches!(current_type, Some(EventType::Lyric)) {
          events.push(Self::synthetic_event(
            events.len(),
            EventType::Lyric,
            Some(current_value),
            linked,
          ));
          current_value = String::new();
          linked = true;
        }

        current_type = None;
        continue;
      }

      if c == BLOCK_OPEN {
        block_level += 1;
      } else if c == BLOCK_CLOSE {
        block_level -= 1;
      }

      current_type = Some(EventType::Lyric);
      current_value.push(c);
    }

    if let Some(EventType::Lyric) = current_type {
      events.push(Self::synthetic_event(
        events.len(),
        EventType::Lyric,
        Some(current_value),
        linked,
      ));
    }

    events
  }
}
