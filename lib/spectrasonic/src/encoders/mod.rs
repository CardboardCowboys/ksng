use std::path::{Path, PathBuf};

pub mod ffmpeg;

#[derive(Debug, Clone)]
pub enum AudioCodec {
  Mp3,
  Aac,
  //Vorbis,
  //Opus,
  Wav,
  //Flac,
}

impl AudioCodec {
  /// Returns the given path with an extension that matches the given codec.
  pub fn set_extension(&self, path: &Path) -> PathBuf {
    path.with_extension(match self {
      AudioCodec::Mp3 => "mp3",
      AudioCodec::Aac => "m4a",
      //AudioCodec::Vorbis => "ogg",
      //AudioCodec::Opus => "opus",
      AudioCodec::Wav => "wav",
      //AudioCodec::Flac => "flac",
    })
  }
}
