use egui::{Context, Id, Modal, Sides};

use crate::{
  modals::{save_project::SaveProjectModal, KModal},
  ui_event::KsngEvent,
  KsngApp,
};

pub struct DirtyWarningModal {
  pub open: bool,
  pub after: KsngEvent,
}

impl DirtyWarningModal {
  pub fn new(after: KsngEvent) -> DirtyWarningModal {
    DirtyWarningModal { open: true, after }
  }
}

impl KModal for DirtyWarningModal {
  fn process(&mut self, app: &KsngApp, context: &Context) {
    if !self.open {
      return;
    }

    let modal = Modal::new(Id::new("modal#dirty_warning")).show(context, |ui| {
      ui.set_width(250.0);

      ui.heading("Save current project?");
      ui.label("The current project has been modified. Would you like to save before closing it?");

      ui.separator();

      Sides::new().show(
        ui,
        |_ui| {},
        |ui| {
          if ui.button("Cancel").clicked() {
            self.open = false;
          }

          if ui.button("Close Without Saving").clicked() {
            app.dispatch(self.after);
            self.open = false;
          }

          if ui.button("Save").clicked() {
            SaveProjectModal::save(app, Some(self.after));
            self.open = false;
          }
        },
      )
    });

    if modal.should_close() {
      self.open = false;
    }
  }

  fn should_cleanup(&self) -> bool {
    !self.open
  }
}
