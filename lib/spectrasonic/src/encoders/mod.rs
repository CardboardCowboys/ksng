use std::path::{Path, PathBuf};

use crate::{Error, PlanarAudioBuffer, chain::AudioInfo};

#[cfg(feature = "ffmpeg")]
pub mod ffmpeg;
#[cfg(feature = "encoder-flac")]
pub mod flac;

#[derive(Debug, Clone)]
pub enum AudioCodec {
  Mp3,
  Aac,
  //Vorbis,
  //Opus,
  Wav,
  Flac,
}

/// Options to pass to an encoder.
#[derive(Debug, Clone)]
pub struct AudioEncoderOptions {
  /// The bit rate to use when encoding. This is not used by all encoders.
  pub bit_rate: usize,
  /// A raw string of options to pass to the encoder. The format of this string
  /// depends on the encoder.
  pub options: String,
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
      AudioCodec::Flac => "flac",
    })
  }
}

pub trait AudioEncoder {
  fn write(&mut self, buffer: &dyn PlanarAudioBuffer) -> Result<(), Error>;
  fn finalize(&mut self) -> Result<(), Error>;
}

pub fn encoder_for_file<P: AsRef<Path>>(
  output: P,
  codec: AudioCodec,
  options: AudioEncoderOptions,
  info: AudioInfo,
) -> Result<Box<dyn AudioEncoder>, Error> {
  #[cfg(feature = "ffmpeg")]
  {
    if matches!(codec, AudioCodec::Aac | AudioCodec::Mp3 | AudioCodec::Wav) {
      use crate::encoders::ffmpeg::FfmpegAudioEncoder;

      return Ok(Box::new(FfmpegAudioEncoder::new(
        output, codec, options, info,
      )?));
    }
  }
  #[cfg(feature = "encoder-flac")]
  {
    if matches!(codec, AudioCodec::Flac) {
      use crate::encoders::flac::FlacAudioEncoder;

      return Ok(Box::new(FlacAudioEncoder::new(output, info)?));
    }
  }

  Err(Error::msg(format!(
    "Cannot find encoder for codec {codec:?}"
  )))
}
