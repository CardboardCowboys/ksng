use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Where the audio data is located.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum AudioFileSource {
  /// A file path on the disk.
  Path(PathBuf),
  /// The audio file can be located in the attachment to the File.
  Attachment(Uuid),
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum AudioFileType {
  Mp3,
  Wave,
  Flac,
  Aac,
  Ogg,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct AudioFile {
  pub id: Uuid,
  pub file_type: AudioFileType,
  pub source: AudioFileSource,
}
