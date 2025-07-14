use std::fmt::Display;

#[derive(Debug)]
pub enum Error {
  Unsupported(String),
  Io(String),
  Serde(String),
  Format(String),
}

impl std::error::Error for Error {}

impl Display for Error {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Error::Unsupported(str) => f.write_str(&format!("Error::Unsupported ({str})")),
      Error::Io(str) => f.write_str(&format!("Error::Io ({str})")),
      Error::Serde(str) => f.write_str(&format!("Error::Serde ({str})")),
      Error::Format(str) => f.write_str(&format!("Error::Format ({str})")),
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
