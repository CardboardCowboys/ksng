use std::cell::RefCell;

use egui::Context;

use crate::KsngApp;

pub trait KModal {
  fn should_cleanup(&self) -> bool;
  fn process(&mut self, app: &KsngApp, context: &Context);
}

#[derive(Default)]
pub struct ModalManager {
  modals: RefCell<Vec<RefCell<Box<dyn KModal>>>>,
  new_modals: RefCell<Vec<RefCell<Box<dyn KModal>>>>,
}

impl ModalManager {
  pub fn add(&self, modal: impl KModal + 'static) {
    self
      .new_modals
      .borrow_mut()
      .push(RefCell::new(Box::new(modal)))
  }

  pub fn process(&self, app: &KsngApp, context: &Context) {
    self
      .modals
      .borrow_mut()
      .append(&mut *self.new_modals.borrow_mut());

    for modal in self.modals.borrow().iter() {
      modal.borrow_mut().process(app, context);
    }

    self
      .modals
      .borrow_mut()
      .retain(|m| !m.borrow().should_cleanup());
  }
}

pub mod alert;
pub mod confirm;
pub mod dirty_warning;
pub mod open_project;
pub mod save_project;
