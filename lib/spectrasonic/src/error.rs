use std::fmt::Display;

#[derive(Debug)]
pub enum RubatoError {
  Construction(rubato::ResamplerConstructionError),
  Resampling(rubato::ResampleError),
}

#[derive(Debug)]
pub enum Error {
  Message(String),
  Io(std::io::Error),
  #[cfg(feature = "symphonia")]
  Symphonia(symphonia::core::errors::Error),
  Rubato(RubatoError),
  #[cfg(feature = "ffmpeg")]
  Ffmpeg(ffmpeg_next::Error),
}

impl Error {
  pub fn msg(s: impl Into<String>) -> Error {
    Error::Message(s.into())
  }
}

impl Display for Error {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{:?}", self)
  }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
  fn from(value: std::io::Error) -> Self {
    Error::Io(value)
  }
}

#[cfg(feature = "symphonia")]
impl From<symphonia::core::errors::Error> for Error {
  fn from(value: symphonia::core::errors::Error) -> Self {
    Error::Symphonia(value)
  }
}

impl From<rubato::ResamplerConstructionError> for Error {
  fn from(value: rubato::ResamplerConstructionError) -> Self {
    Error::Rubato(RubatoError::Construction(value))
  }
}

impl From<rubato::ResampleError> for Error {
  fn from(value: rubato::ResampleError) -> Self {
    Error::Rubato(RubatoError::Resampling(value))
  }
}

impl From<audioadapter_buffers::SizeError> for Error {
  fn from(value: audioadapter_buffers::SizeError) -> Self {
    Error::Message(format!("audioadapter_buffers::SizeError({})", value))
  }
}

#[cfg(feature = "ffmpeg")]
impl From<ffmpeg_next::Error> for Error {
  fn from(value: ffmpeg_next::Error) -> Self {
    Error::Ffmpeg(value)
  }
}
