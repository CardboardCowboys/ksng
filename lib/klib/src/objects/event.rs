use binary_rw::{BinaryReader, BinaryWriter};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{error::Error, objects::audio::AudioFile, timecode::Timecode};

use super::audio::AudioFileSource;

#[derive(Serialize, Deserialize, PartialEq, Eq)]
pub enum EventValue {
  Lyric { text: String },
  AudioClip { offset: Timecode, file: AudioFile },
}

#[repr(u8)]
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum EventType {
  #[default]
  Lyric = 0,
  LineBreak = 1,
  ParagraphBreak = 2,
  AudioClip = 8,
  Image = 16,
}

impl EventType {
  pub fn read(reader: &mut BinaryReader) -> Result<(EventType, Option<EventValue>), Error> {
    let type_byte = reader.read_u8()?;
    let event_type = match type_byte {
      0 => Ok(EventType::Lyric),
      1 => Ok(EventType::LineBreak),
      2 => Ok(EventType::ParagraphBreak),
      8 => Ok(EventType::AudioClip),
      16 => Ok(EventType::Image),
      _ => Err(Error::Format(format!("Unknown event type {type_byte}"))),
    }?;

    let event_value: Option<EventValue> = serde_json::from_str(&reader.read_string()?)?;

    Ok((event_type, event_value))
  }
}

/// Events are the core data type of a Karaoke file.
///
/// An event can be an audio clip, a lyric or part of a lyric, graphical elements,
/// or any other data that has a start and end time.
#[derive(Default, PartialEq, Eq)]
pub struct Event {
  /// The unique ID of this event.
  pub id: Uuid,
  /// The ID of the event this is linked to.
  ///
  /// Lyrics of words with multiple syllables are represented as multiple events,
  /// with the first event having no `linked_id` and following events having the
  /// `linked_id` of the previous event.
  pub linked_id: Option<Uuid>,
  /// The start time of this event.
  pub start_timecode: Timecode,
  /// The end time of this event.
  pub end_timecode: Timecode,
  /// The type of this event.
  pub event_type: EventType,
  /// Any value associated with an event of this type.
  pub value: Option<EventValue>,
}

impl PartialOrd for Event {
  fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
    Some(self.cmp(other))
  }
}

impl Ord for Event {
  fn cmp(&self, other: &Self) -> std::cmp::Ordering {
    self.start_timecode.0.cmp(&other.start_timecode.0)
  }
}

impl Event {
  pub fn new_lyric(start: Timecode, end: Timecode, text: String) -> Event {
    Event {
      id: Uuid::new_v4(),
      linked_id: None,
      start_timecode: start,
      end_timecode: end,
      event_type: EventType::Lyric,
      value: Some(EventValue::Lyric { text }),
    }
  }

  pub fn new_audio(start: Timecode, end: Timecode, offset: Timecode, file: AudioFile) -> Event {
    Event {
      id: Uuid::new_v4(),
      linked_id: None,
      start_timecode: start,
      end_timecode: end,
      event_type: EventType::AudioClip,
      value: Some(EventValue::AudioClip { offset, file }),
    }
  }

  pub fn write(&self, writer: &mut BinaryWriter) -> Result<(), Error> {
    let (hi, lo) = self.id.as_u64_pair();
    writer.write_u64(hi)?;
    writer.write_u64(lo)?;
    writer.write_bool(self.linked_id.is_some())?;
    if let Some(linked_id) = self.linked_id {
      let (hi, lo) = linked_id.as_u64_pair();
      writer.write_u64(hi)?;
      writer.write_u64(lo)?;
    }

    writer.write_u32(self.start_timecode.0)?;
    writer.write_u32(self.end_timecode.0)?;
    writer.write_u8(self.event_type as u8)?;
    writer.write_string(serde_json::to_string(&self.value)?)?;

    Ok(())
  }

  pub fn read(reader: &mut BinaryReader) -> Result<Event, Error> {
    let mut event = Event::default();

    let hi = reader.read_u64()?;
    let lo = reader.read_u64()?;
    event.id = Uuid::from_u64_pair(hi, lo);

    let has_linked_id = reader.read_bool()?;
    event.linked_id = match has_linked_id {
      true => {
        let hi = reader.read_u64()?;
        let lo = reader.read_u64()?;
        Some(Uuid::from_u64_pair(hi, lo))
      }
      false => None,
    };

    event.start_timecode = Timecode(reader.read_u32()?);
    event.end_timecode = Timecode(reader.read_u32()?);

    let (event_type, event_value) = EventType::read(reader)?;
    event.event_type = event_type;
    event.value = event_value;

    Ok(event)
  }

  /// Checks whether this event is within the range (start, end)
  pub fn is_in_range(&self, range: (Timecode, Timecode)) -> bool {
    Timecode::ranges_overlap(range, (self.start_timecode, self.end_timecode))
  }

  /// Obtains a string describing this event, if any.
  /// For example, a Lyric event will return the text of the lyric.
  pub fn description(&self) -> Option<String> {
    self.value.as_ref().and_then(|v| match v {
      EventValue::Lyric { text } => Some(if self.linked_id.is_some() {
        "-".to_string() + text
      } else {
        text.clone()
      }),
      EventValue::AudioClip { file, .. } => match &file.source {
        AudioFileSource::Path(path_buf) => path_buf
          .file_stem()
          .and_then(|s| s.to_str())
          .map(|s| s.to_owned()),
        AudioFileSource::Managed => None,
      },
    })
  }

  /// Returns the text of this lyric event, or an empty string if not a lyric event.
  pub fn text(&self) -> Option<&str> {
    self.value.as_ref().map(|v| match v {
      EventValue::Lyric { text } => text.as_str(),
      _ => "",
    })
  }
}
