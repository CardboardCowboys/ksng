use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Where this audio file should be expected to be located.
#[derive(Serialize, Deserialize)]
pub enum AudioFileSource {
  /// The audio file is located at this path.
  Path(PathBuf),
  /// The audio file storage is managed by the libary user,
  /// and only the file ID and type will be necessary to
  /// retrieve it.
  Managed,
}

#[derive(Serialize, Deserialize)]
pub enum AudioFileType {
  Mp3,
  Wave,
  Flac,
  Aac,
  Ogg,
  Opus,
}

#[derive(Serialize, Deserialize)]
pub struct AudioFile {
  id: Uuid,
  file_type: AudioFileType,
  source: AudioFileSource,
}
