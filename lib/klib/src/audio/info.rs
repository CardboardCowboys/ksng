use std::path::Path;

use crate::{objects::audio::AudioFileType, timecode::Timecode};

pub struct AudioFileInfo {
  pub audio_type: AudioFileType,
  pub length: Timecode,
}

impl AudioFileInfo {
  pub fn from_file(path: &Path) -> Result<Option<AudioFileInfo>, crate::error::Error> {
    let info = spectrasonic::info::get_file_info(path)?;
    Ok(info.and_then(|info| {
      Some(AudioFileInfo {
        length: info.duration.into(),
        audio_type: Self::file_type_from_mime(&info.mime_type)?,
      })
    }))
  }

  fn file_type_from_mime(mime: &str) -> Option<AudioFileType> {
    if mime == "audio/mpeg" {
      return Some(AudioFileType::Mp3);
    } else if mime == "audio/x-flac" {
      return Some(AudioFileType::Flac);
    } else if mime == "audio/ogg" {
      return Some(AudioFileType::Ogg);
    } else if mime == "audio/x-wav" {
      return Some(AudioFileType::Wave);
    } else if mime == "audio/aac" {
      return Some(AudioFileType::Aac);
    }

    None
  }
}
