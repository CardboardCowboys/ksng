use std::sync::{Arc, RwLock};

use log::{Level, error};

use crate::util::error::UiError;

pub enum LogType {
  Trace,
  Debug,
  Info,
  Warning,
  Error,
}

pub struct LogMessage {
  pub text: String,
  pub log_type: LogType,
  pub time: chrono::NaiveDateTime,
}

#[derive(Clone)]
pub struct Logger {
  pub messages: Arc<RwLock<Vec<LogMessage>>>,
  colog: Arc<Box<dyn log::Log>>,
}

impl Default for Logger {
  fn default() -> Self {
    let colog = colog::default_builder().build();
    Self {
      messages: Default::default(),
      colog: Arc::new(Box::new(colog)),
    }
  }
}

impl Logger {
  pub fn wrap<T>(val: Result<T, impl Into<UiError>>) -> Option<T> {
    match val {
      Ok(v) => Some(v),
      Err(e) => {
        let ui_error: UiError = e.into();
        error!("{ui_error:?}");
        None
      }
    }
  }
}

impl log::Log for Logger {
  fn enabled<'a>(&self, metadata: &log::Metadata<'a>) -> bool {
    !matches!(metadata.level(), Level::Trace | Level::Debug)
  }

  fn log<'a>(&self, record: &log::Record<'a>) {
    let t: LogType = match record.level() {
      Level::Trace => LogType::Trace,
      Level::Debug => LogType::Debug,
      Level::Info => LogType::Info,
      Level::Warn => LogType::Warning,
      Level::Error => LogType::Error,
    };

    self.messages.write().unwrap().push(LogMessage {
      text: record.args().to_string(),
      log_type: t,
      time: chrono::offset::Local::now().naive_local(),
    });

    self.colog.log(record);
  }

  fn flush(&self) {}
}
