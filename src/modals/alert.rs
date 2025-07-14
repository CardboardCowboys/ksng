use crate::modals::KModal;
use egui::{Id, Modal};

pub struct AlertModal {
  open: bool,
  text: String,
}

impl AlertModal {
  pub fn new(text: String) -> Self {
    AlertModal { text, open: true }
  }
}

impl KModal for AlertModal {
  fn should_cleanup(&self) -> bool {
    !self.open
  }

  fn process(&mut self, _app: &crate::KsngApp, context: &egui::Context) {
    if !self.open {
      return;
    }

    let modal = Modal::new(Id::new("modal#confirm")).show(context, |ui| {
      ui.set_width(250.0);

      ui.label(&self.text);

      ui.separator();

      ui.vertical_centered(|ui| {
        if ui.button("OK").clicked() {
          self.open = false;
        }
      });
    });

    if modal.should_close() {
      self.open = false;
    }
  }
}
