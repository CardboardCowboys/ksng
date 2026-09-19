pub mod buffer;
pub mod chain;
#[cfg(feature = "encoding")]
pub mod encoders;
pub mod error;
pub mod filters;
pub mod info;
pub mod sources;

pub use buffer::PlanarAudioBuffer;
pub use chain::{AudioChain, AudioFilter, AudioSource};
pub use error::Error;

#[derive(Debug, Clone, Copy)]
pub struct Timecode {
  pub samples: usize,
  pub sample_rate: usize,
}

impl Timecode {
  pub const fn from_ms(ms: u32) -> Timecode {
    Timecode {
      samples: ms as usize,
      sample_rate: 1000,
    }
  }

  pub const fn to_seconds(&self) -> f32 {
    self.samples as f32 / self.sample_rate as f32
  }

  pub const fn to_seconds_f64(&self) -> f64 {
    self.samples as f64 / self.sample_rate as f64
  }
}
