use egui::{Button, Id, Modal, Sides};

use crate::{data::Data, modals::KModal, ui_event::KsngEvent, KsngApp};

pub struct SaveProjectModal {
  open: bool,
  after: Option<KsngEvent>,
  name: String,
}

impl SaveProjectModal {
  pub fn save(app: &KsngApp, after: Option<KsngEvent>) {
    if let Some(project) = app.project.read().expect("Poisoned").as_ref() {
      if project.name.is_none() {
        app.modals.add(SaveProjectModal {
          open: true,
          after,
          name: String::new(),
        });
        return;
      }

      SaveProjectModal::perform_async_save(app, after);
    } else {
      app.set_dirty_state(false);

      if let Some(after) = after {
        app.dispatch(after);
      }
    }
  }

  fn perform_async_save(app: &KsngApp, after: Option<KsngEvent>) {
    app.set_dirty_state(false);
    let event_queue = app.event_queue.clone();
    let data = app.data.clone();
    let project_holder = app.project.clone();
    app.async_handler.clone().wrap(async move || {
      let project_rw = project_holder.read().expect("Poisoned").clone();
      if let Some(project) = project_rw.as_ref() {
        data.save_project(&project).await?;
      }
      if let Some(after) = after {
        event_queue.write().expect("Poisoned").push_back(after);
      }
      Ok(())
    });
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
            if let Some(project) = app.project.write().expect("Poisoned").as_mut() {
              project.name = Some(self.name.clone());
              self.open = false;
            }

            Self::perform_async_save(app, self.after);
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
