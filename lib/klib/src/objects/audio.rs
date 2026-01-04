use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Where the audio data is located.
#[derive(Serialize, Deserialize, PartialEq, Eq)]
pub enum AudioFileSource {
  /// A file path on the disk.
  Path(PathBuf),
  /// The audio data is managed by the consumer of the library and can be looked
  /// up solely based on the ID and file type.
  Managed,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum AudioFileType {
  Mp3,
  Wave,
  Flac,
  Aac,
  Ogg,
}

#[derive(Serialize, Deserialize, PartialEq, Eq)]
pub struct AudioFile {
  pub id: Uuid,
  pub file_type: AudioFileType,
  pub source: AudioFileSource,
}
