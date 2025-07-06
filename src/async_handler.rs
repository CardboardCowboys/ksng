use std::{
  convert::Into,
  future::Future,
  sync::{
    atomic::{AtomicU8, AtomicUsize, Ordering},
    Arc,
  },
};

use egui::{Id, Modal, Spinner};
use std::sync::RwLock;

use crate::{
  error::UiError,
  logger::{LogType, Logger},
};

/// Handles one-off async tasks that are not meant to be long-running and
/// do not need to return a value, but for which state should not continue to
/// be modified until they complete.
pub struct AsyncHandler {
  pending_futures: AtomicUsize,
  logger: Arc<Logger>,
}

impl AsyncHandler {
  /// Creates a new AsyncHandler.
  pub fn new(logger: Arc<Logger>) -> AsyncHandler {
    AsyncHandler {
      pending_futures: AtomicUsize::new(0),
      logger,
    }
  }

  /// Wraps the given async closure.
  pub fn wrap<F, Fut>(self: Arc<AsyncHandler>, f: F)
  where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = Result<(), UiError>> + Send,
  {
    self.pending_futures.fetch_add(1, Ordering::Relaxed);
    tokio::spawn(async move {
      let result = f().await;
      if let Err(e) = result {
        self.logger.log(LogType::Error, format!("{e:?}"));
      }

      self.pending_futures.fetch_sub(1, Ordering::Relaxed);
    });
  }

  /// Runs UI updates for async handler (displaying the loading spinner as needed).
  pub fn update(&self, context: &egui::Context) {
    if self.pending_futures.load(Ordering::Relaxed) > 0 {
      Modal::new(Id::new("modal#loading")).show(context, |ui| {
        ui.add(Spinner::new().size(128.0));
      });
    }
  }
}

#[derive(PartialEq, Copy, Clone)]
#[repr(u8)]
pub enum AsyncValueState {
  Unloaded = 0,
  Loading = 1,
  Loaded = 2,
}

impl From<AsyncValueState> for u8 {
  fn from(val: AsyncValueState) -> Self {
    val as u8
  }
}

impl From<u8> for AsyncValueState {
  fn from(value: u8) -> Self {
    match value {
      1 => AsyncValueState::Loading,
      2 => AsyncValueState::Loaded,
      _ => AsyncValueState::Unloaded,
    }
  }
}

struct AsyncValueImpl<T> {
  state: AtomicU8,
  value: RwLock<Arc<Option<T>>>,
}

#[derive(Clone)]
pub struct AsyncValue<T>
where
  T: Send + Sync + 'static,
{
  inner: Arc<AsyncValueImpl<T>>,
  logger: Arc<Logger>,
}

impl<T> AsyncValue<T>
where
  T: Send + Sync + 'static,
{
  pub fn new(logger: Arc<Logger>) -> AsyncValue<T> {
    AsyncValue {
      inner: Arc::new(AsyncValueImpl {
        state: AtomicU8::new(AsyncValueState::Unloaded.into()),
        value: Default::default(),
      }),
      logger,
    }
  }

  pub fn get(&self) -> Arc<Option<T>> {
    match Into::<AsyncValueState>::into(self.inner.state.load(Ordering::Relaxed)) {
      AsyncValueState::Unloaded | AsyncValueState::Loading => Arc::new(None),
      AsyncValueState::Loaded => self.inner.value.read().expect("Reading AsyncValue").clone(),
    }
  }

  pub fn state(&self) -> AsyncValueState {
    self.inner.state.load(Ordering::Relaxed).into()
  }

  pub fn set(&self, val: Option<T>) {
    self
      .inner
      .state
      .store(AsyncValueState::Loaded.into(), Ordering::Relaxed);
    *self.inner.value.write().expect("Writing AsyncValue") = Arc::new(val);
  }

  pub fn load<F, Fut>(&self, f: F)
  where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = Result<T, UiError>> + Send,
  {
    self
      .inner
      .state
      .store(AsyncValueState::Loading.into(), Ordering::Relaxed);
    let logger = self.logger.clone();
    let inner = self.inner.clone();

    tokio::spawn(async move {
      let result = logger.wrap(f().await);
      if let Some(val) = result {
        *inner.value.write().expect("Writing AsyncValue") = Arc::new(Some(val));
        inner
          .state
          .store(AsyncValueState::Loaded.into(), Ordering::Relaxed);
      } else if inner.value.read().expect("Reading AsyncValue").is_none() {
        inner
          .state
          .store(AsyncValueState::Unloaded.into(), Ordering::Relaxed);
      }
    });
  }
}
