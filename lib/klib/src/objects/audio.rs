use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
  extension: AudioFileType,
}
