use crate::{AudioSource, Error, Timecode, chain::AudioChainBuilder};
use std::path::Path;

#[cfg(feature = "symphonia")]
mod symphonia;

#[cfg(feature = "symphonia")]
pub use symphonia::SymphoniaAudioSource;

#[cfg(feature = "ffmpeg")]
mod ffmpeg;

#[cfg(feature = "ffmpeg")]
pub use ffmpeg::FfmpegAudioSource;

/// Returns the best available audio source to read `file`.
///
/// If the `ffmpeg` feature is enabled, an `FfmpegAudioSource` is returned.
/// If the `symphonia` feature is enabled, a `SymphoniaAudioSource` is returned.
/// Else, a `NullAudioSource` is returned (and a warning is logged).
pub fn source_for_file<P: AsRef<Path>>(file: P) -> Result<Box<dyn AudioSource>, Error> {
  #[cfg(feature = "ffmpeg")]
  {
    Ok(Box::new(FfmpegAudioSource::new(file)?))
  }
  #[cfg(all(feature = "symphonia", not(feature = "ffmpeg")))]
  {
    Ok(Box::new(SymphoniaAudioSource::new(file)?))
  }
  #[cfg(all(not(feature = "symphonia"), not(feature = "ffmpeg")))]
  {
    log::warn!(
      "spectrasonic has been built without any audio source features enabled - returning NullAudioSource from source_for_file"
    );
    Ok(Box::new(NullAudioSource {
      channels: 2,
      sample_rate: 44100,
    }))
  }
}

/// An `AudioSource` that produces infinite silence.
pub struct NullAudioSource {
  pub channels: usize,
  pub sample_rate: usize,
}

impl AudioSource for NullAudioSource {
  fn read(&mut self, buffer: &mut dyn crate::PlanarAudioBuffer) -> Result<usize, Error> {
    for ch in 0..buffer.num_channels() {
      buffer.channel_mut(ch).fill(0.0_f32);
    }
    Ok(buffer.num_frames())
  }

  fn seek(&mut self, _pos: crate::Timecode) -> Result<(), Error> {
    Ok(())
  }

  fn duration(&self) -> crate::Timecode {
    Timecode {
      samples: usize::MAX,
      sample_rate: self.sample_rate,
    }
  }

  fn info(&self) -> crate::chain::AudioInfo {
    crate::chain::AudioInfo {
      num_channels: self.channels,
      sample_rate: self.sample_rate,
    }
  }

  fn builder(self: Box<Self>) -> crate::chain::AudioChainBuilder {
    AudioChainBuilder::new(self)
  }
}
