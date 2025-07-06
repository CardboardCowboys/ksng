use binary_rw::{BinaryReader, BinaryWriter};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
  error::Error,
  objects::{event::Event, file::File},
};

/// Settings for an audio track.
#[derive(Serialize, Deserialize)]
pub struct AudioTrackValue {
  /// Whether this track should be muted.
  pub muted: bool,
  /// The volume of this track, between 0 and 1.
  pub volume: f32,
}

impl Default for AudioTrackValue {
  fn default() -> Self {
    Self {
      muted: false,
      volume: 1.0,
    }
  }
}

/// Type-specific data for a track.
#[derive(Serialize, Deserialize)]
pub enum TrackValue {
  /// Settings for an audio track.
  Audio(AudioTrackValue),
}

/// The type of the track.
#[repr(u8)]
#[derive(Clone, Copy, Default, Debug)]
pub enum TrackType {
  /// A lyrics track.
  #[default]
  Lyrics = 0,
  /// An audio track.
  Audio = 1,
}

impl TrackType {
  /// Reads this `TrackType` from the `BinaryReader` along with the corresponding value, if any.
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

/// A single track in a file containing events.
#[derive(Default)]
pub struct Track {
  /// The unique ID of the track.
  pub id: Uuid,
  /// The order of the track in relation to other tracks.
  pub order: u32,
  /// The type of the track.
  pub track_type: TrackType,
  /// Any data of this track specific to its type.
  pub track_value: Option<TrackValue>,

  /// The events on this track.
  pub events: Vec<Event>,
}

impl Track {
  /// Creates a new track of type `track_type` with the given `order`.
  pub fn new_type(track_type: TrackType, order: u32) -> Track {
    match track_type {
      TrackType::Lyrics => Track::new_lyrics(order),
      TrackType::Audio => Track::new_audio(order),
    }
  }

  /// Creates a new `Lyrics` track with the given `order`.
  pub fn new_lyrics(order: u32) -> Track {
    Track {
      id: Uuid::new_v4(),
      order,
      track_type: TrackType::Lyrics,
      track_value: None,
      events: Default::default(),
    }
  }

  /// Creates a new `Audio` track with the given `order`.
  pub fn new_audio(order: u32) -> Track {
    Track {
      id: Uuid::new_v4(),
      order,
      track_type: TrackType::Audio,
      track_value: Some(TrackValue::Audio(AudioTrackValue::default())),
      events: Default::default(),
    }
  }

  /// Writes this track to the provided `BinaryWriter`.
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

  /// Reads this track from the provided `BinaryReader`
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
