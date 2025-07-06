use binary_rw::{BinaryReader, BinaryWriter};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
  error::Error,
  objects::{event::Event, file::File},
};

#[derive(Serialize, Deserialize, Clone)]
pub struct AudioTrackValue {
  pub muted: bool,
  pub volume: f32,
}

#[derive(Serialize, Deserialize, Clone)]
pub enum TrackValue {
  Audio(AudioTrackValue),
}

#[repr(u8)]
#[derive(Clone, Copy, Default)]
pub enum TrackType {
  #[default]
  Lyrics = 0,
  Audio = 1,
}

impl TrackType {
  pub fn read(reader: &mut BinaryReader) -> Result<(TrackType, Option<TrackValue>), Error> {
    let type_byte = reader.read_u8()?;
    let track_type = match type_byte {
      0 => Ok(TrackType::Lyrics),
      1 => Ok(TrackType::Audio),
      _ => Err(Error::Format(format!("Invalid track type {type_byte}"))),
    }?;

    let track_value: Option<TrackValue> = match track_type {
      TrackType::Audio => serde_json::from_str(&reader.read_string()?)?,
      _ => None,
    };

    Ok((track_type, track_value))
  }
}

#[derive(Default, Clone)]
pub struct Track {
  pub id: Uuid,
  pub order: u32,
  pub track_type: TrackType,
  pub track_value: Option<TrackValue>,

  pub events: Vec<Event>,
}

impl Track {
  pub fn write(&self, writer: &mut BinaryWriter) -> Result<(), Error> {
    let (hi, lo) = self.id.as_u64_pair();
    writer.write_u64(hi)?;
    writer.write_u64(lo)?;
    writer.write_u32(self.order)?;

    writer.write_u8(self.track_type as u8)?;
    writer.write_string(serde_json::to_string(&self.track_value)?)?;

    writer.write_usize(self.events.len())?;
    for event in &self.events {
      event.write(writer)?;
    }

    Ok(())
  }

  pub fn read(reader: &mut BinaryReader) -> Result<Track, Error> {
    let mut track = Track::default();

    let hi = reader.read_u64()?;
    let lo = reader.read_u64()?;

    track.id = Uuid::from_u64_pair(hi, lo);
    track.order = reader.read_u32()?;

    let (track_type, track_value) = TrackType::read(reader)?;
    track.track_type = track_type;
    track.track_value = track_value;

    let event_num = reader.read_usize()?;
    track.events.reserve(event_num);

    for _i in 0..event_num {
      track.events.push(Event::read(reader)?)
    }

    Ok(track)
  }
}
