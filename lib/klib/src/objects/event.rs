use binary_rw::{BinaryReader, BinaryWriter};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{error::Error, objects::audio::AudioFile, timecode::Timecode};

#[derive(Serialize, Deserialize, Clone)]
pub enum EventValue {
  Lyric { text: String },
  AudioClip { offset: Timecode, file: AudioFile },
}

#[repr(u8)]
#[derive(Clone, Copy, Default)]
pub enum EventType {
  #[default]
  Lyric = 0,
  LineBreak = 1,
  AudioClip = 8,
  Image = 16,
}

impl EventType {
  pub fn read(reader: &mut BinaryReader) -> Result<(EventType, Option<EventValue>), Error> {
    let type_byte = reader.read_u8()?;
    let event_type = match type_byte {
      0 => Ok(EventType::Lyric),
      1 => Ok(EventType::LineBreak),
      8 => Ok(EventType::AudioClip),
      16 => Ok(EventType::Image),
      _ => Err(Error::Format(format!("Unknown event type {type_byte}"))),
    }?;

    let event_value: Option<EventValue> = match event_type {
      EventType::Lyric | EventType::AudioClip => serde_json::from_str(&reader.read_string()?)?,
      _ => None,
    };

    Ok((event_type, event_value))
  }
}

#[derive(Default, Clone)]
pub struct Event {
  pub id: Uuid,
  pub linked_id: Option<Uuid>,
  pub start_timecode: Timecode,
  pub end_timecode: Timecode,
  pub event_type: EventType,
  pub value: Option<EventValue>,
}

impl Event {
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
}
