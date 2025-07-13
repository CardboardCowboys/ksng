use egui::{Button, Id, Modal, Sides};

use crate::{fs::Data, modals::KModal, util::ui_event::KsngEvent, KsngApp};

pub struct SaveProjectModal {
  open: bool,
  after: Option<KsngEvent>,
  name: String,
}

impl SaveProjectModal {
  pub fn save(app: &KsngApp, after: Option<KsngEvent>) {
    if let Some(project) = &*app.project.borrow() {
      if project.name.is_none() {
        app.modals.add(SaveProjectModal {
          open: true,
          after,
          name: String::new(),
        });
        return;
      }

      app.logger.wrap(Data::save_project(project));
    }

    app.set_dirty_state(false);

    if let Some(after) = after {
      app.dispatch(after);
    }
  }
}

impl KModal for SaveProjectModal {
  fn process(&mut self, app: &KsngApp, context: &egui::Context) {
    if !self.open {
      return;
    }

    let modal = Modal::new(Id::new("modal#save_project")).show(context, |ui| {
      ui.set_width(250.0);

      ui.heading("Save Project");
      ui.label("Enter a name for the project");
      ui.text_edit_singleline(&mut self.name);

      ui.separator();

      let validate_name = !self.name.trim().is_empty();

      Sides::new().show(
        ui,
        |_ui| {},
        |ui| {
          if ui.add_enabled(validate_name, Button::new("Save")).clicked() {
            if let Some(project) = &mut *app.project.borrow_mut() {
              project.name = Some(self.name.clone());
              app.logger.wrap(Data::save_project(project));
              project.dirty = false;
              self.open = false;
              if let Some(after) = self.after {
                app.dispatch(after);
              }
            }
          }

          if ui.button("Cancel").clicked() {
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
