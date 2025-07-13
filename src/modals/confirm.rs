use egui::{Id, Modal, Sides};

use crate::{modals::KModal, util::ui_event::KsngEvent};

pub struct ConfirmModal {
  open: bool,
  heading: String,
  text: String,
  after: KsngEvent,
}

impl ConfirmModal {
  pub fn new(heading: String, text: String, after: KsngEvent) -> Self {
    ConfirmModal {
      heading,
      text,
      open: true,
      after,
    }
  }
}

impl KModal for ConfirmModal {
  fn should_cleanup(&self) -> bool {
    !self.open
  }

  fn process(&mut self, app: &crate::KsngApp, context: &egui::Context) {
    if !self.open {
      return;
    }

    let modal = Modal::new(Id::new("modal#confirm")).show(context, |ui| {
      ui.set_width(250.0);

      ui.heading(&self.heading);
      ui.label(&self.text);

      Sides::new().show(
        ui,
        |_ui| {},
        |ui| {
          if ui.button("No").clicked() {
            self.open = false;
          }

          if ui.button("Yes").clicked() {
            self.open = false;
            app.dispatch(self.after);
          }
        },
      )
    });

    if modal.should_close() {
      self.open = false;
    }
  }
}
