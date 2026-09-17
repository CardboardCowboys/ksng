use std::fmt::Display;

#[derive(Debug, Clone)]
pub enum Error {
  Unsupported(String),
  Io(String),
  Serde(String),
  Format(String),
  Layout(String),
  Skia(String),
  Audio(String),
  VideoExport(String),
  Spectrasonic(String),
}

impl std::error::Error for Error {}

impl Display for Error {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Error::Unsupported(str) => f.write_str(&format!("Error::Unsupported ({str})")),
      Error::Io(str) => f.write_str(&format!("Error::Io ({str})")),
      Error::Serde(str) => f.write_str(&format!("Error::Serde ({str})")),
      Error::Format(str) => f.write_str(&format!("Error::Format ({str})")),
      Error::Layout(str) => f.write_str(&format!("Error::Layout ({str})")),
      Error::Skia(str) => f.write_str(&format!("Error::Skia ({str})")),
      Error::Audio(str) => f.write_str(&format!("Error::Audio ({str})")),
      Error::VideoExport(str) => f.write_str(&format!("Error::VideoExport ({str})")),
      Error::Spectrasonic(str) => f.write_str(&format!("Error::Spectrasonic ({str})")),
    }
  }
}

impl From<binary_rw::BinaryError> for Error {
  fn from(value: binary_rw::BinaryError) -> Self {
    Error::Io(format!("Binary read/write error: {value:?}"))
  }
}

impl From<serde_json::Error> for Error {
  fn from(value: serde_json::Error) -> Self {
    Error::Serde(format!(
      "JSON serialization/deserialization error: {value:?}"
    ))
  }
}

impl From<std::io::Error> for Error {
  fn from(value: std::io::Error) -> Self {
    Error::Io(format!("{value:?}"))
  }
}

#[cfg(feature = "audio")]
impl From<symphonia::core::errors::Error> for Error {
  fn from(value: symphonia::core::errors::Error) -> Self {
    Error::Audio(format!("Symphonia error: {value:?}"))
  }
}

#[cfg(feature = "export_video")]
impl From<ffmpeg_next::Error> for Error {
  fn from(value: ffmpeg_next::Error) -> Self {
    Error::VideoExport(format!("FFmpeg error: {value:?}"))
  }
}

impl From<spectrasonic::Error> for Error {
  fn from(value: spectrasonic::Error) -> Self {
    Error::Spectrasonic(format!("Spectrasonic error: {value:?}"))
  }
}
