#[derive(Debug)]
pub enum UiError {
  Io(String),
  Serde(String),
  Klib(klib::error::Error),
  InvalidCommand(String),
  Audio(String),
  Image(String),
}

impl From<std::io::Error> for UiError {
  fn from(value: std::io::Error) -> Self {
    UiError::Io(format!("UiError::Io ({value:?})"))
  }
}

impl From<serde_json::Error> for UiError {
  fn from(value: serde_json::Error) -> Self {
    UiError::Serde(format!("UiError::Serde ({value:?})"))
  }
}

impl From<klib::error::Error> for UiError {
  fn from(value: klib::error::Error) -> Self {
    UiError::Klib(value)
  }
}

impl From<symphonia::core::errors::Error> for UiError {
  fn from(value: symphonia::core::errors::Error) -> Self {
    UiError::Audio(format!("UiError::Audio ({value:?})"))
  }
}
