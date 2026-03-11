use std::cell::RefCell;

use egui::Context;

use crate::KsngApp;

/// A KWindow is like a KModal but they do not claim exclusive focus.
pub trait KWindow {
  fn should_cleanup(&self) -> bool;
  fn process(&mut self, app: &KsngApp, context: &Context);
  /// A window can optionally provide a hash of its unique properties.
  /// If an existing window is present with the same hash, a new one will not be
  /// created.
  fn unique_value(&self) -> Option<u64> {
    None
  }
  // Instructs the window to request focus next frame.
  fn request_focus(&mut self);
}

#[derive(Default)]
pub struct WindowManager {
  windows: RefCell<Vec<RefCell<Box<dyn KWindow>>>>,
  new_windows: RefCell<Vec<RefCell<Box<dyn KWindow>>>>,
}

impl WindowManager {
  pub fn add(&self, window: impl KWindow + 'static) {
    // If there already exists a window with this unique value, focus that one
    // instead of creating a new one.
    if let Some(unique_value) = window.unique_value() {
      for window in self.windows.borrow().iter() {
        if let Some(other_unique) = window.borrow().unique_value()
          && other_unique == unique_value
        {
          window.borrow_mut().request_focus();
          return;
        }
      }
    }

    self
      .new_windows
      .borrow_mut()
      .push(RefCell::new(Box::new(window)))
  }

  pub fn process(&self, app: &KsngApp, context: &Context) {
    self
      .windows
      .borrow_mut()
      .append(&mut *self.new_windows.borrow_mut());

    for window in self.windows.borrow().iter() {
      window.borrow_mut().process(app, context);
    }

    self
      .windows
      .borrow_mut()
      .retain(|w| !w.borrow().should_cleanup());
  }

  pub fn clear(&self) {
    self.windows.borrow_mut().clear();
    self.new_windows.borrow_mut().clear();
  }
}

pub mod preferences;
pub mod sync;
pub mod track_config;
