use std::sync::{Arc, RwLock};

use log::error;

use crate::error::UiError;

pub enum LogType {
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

#[derive(Default, Clone)]
pub struct Logger {
  pub messages: Arc<RwLock<Vec<LogMessage>>>,
}

impl Logger {
  pub fn wrap<T>(&self, val: Result<T, UiError>) -> Option<T> {
    match val {
      Ok(v) => Some(v),
      Err(e) => {
        error!("{e:?}");
        self.log(LogType::Error, format!("{e:?}"));
        None
      }
    }
  }

  pub fn log(&self, log_type: LogType, text: String) {
    self.messages.write().unwrap().push(LogMessage {
      text,
      log_type,
      time: chrono::offset::Local::now().naive_local(),
    })
  }
}
