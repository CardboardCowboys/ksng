use egui::{
  Align, Button, Color32, Id, ImageButton, Label, Layout, Modal, Sense, Sides, TopBottomPanel,
};
use egui_extras::{Column, TableBuilder};
use uuid::Uuid;

use crate::{
  fs::Data,
  icons,
  modals::{alert::AlertModal, confirm::ConfirmModal, KModal},
  ui_event::KsngEvent,
};

pub struct OpenProjectModal {
  open: bool,
  selected_id: Option<Uuid>,
}

impl OpenProjectModal {
  pub fn new() -> OpenProjectModal {
    OpenProjectModal {
      open: true,
      selected_id: None,
    }
  }
}

impl KModal for OpenProjectModal {
  fn should_cleanup(&self) -> bool {
    !self.open
  }

  fn process(&mut self, app: &crate::KsngApp, context: &egui::Context) {
    if !self.open {
      return;
    }

    let modal = Modal::new(Id::new("modal#open_project")).show(context, |ui| {
      ui.set_width(400.0);
      ui.set_min_height(400.0);

      ui.vertical(|ui| {
        let manifest = app.logger.wrap(Data::list_projects()).unwrap_or_default();

        if self.selected_id.is_some()
          && !manifest
            .entries
            .iter()
            .any(|p| p.id == self.selected_id.unwrap())
        {
          self.selected_id = None;
        }

        let available_height = ui.available_height();

        let table = TableBuilder::new(ui)
          .striped(true)
          .cell_layout(Layout::left_to_right(Align::Center))
          .column(Column::auto())
          .column(Column::remainder())
          .min_scrolled_height(0.0)
          .max_scroll_height(available_height)
          .sense(Sense::click());

        table
          .header(20.0, |mut header| {
            header.col(|ui| {
              ui.strong("Name");
            });
            header.col(|ui| {
              ui.strong("Last Modified");
            });
          })
          .body(|mut body| {
            for entry in &manifest.entries {
              body.row(18.0, |mut row| {
                row.set_selected(self.selected_id.map(|id| id == entry.id).unwrap_or(false));

                let mut clicked = false;

                row.col(|ui| {
                  if crate::ui::inert_label(ui, &entry.name).clicked() {
                    clicked = true;
                  }
                });

                row.col(|ui| {
                  let mut s = String::new();
                  let _ = entry
                    .last_modified
                    .format("%B %d %Y %I:%M %p")
                    .write_to(&mut s);
                  if crate::ui::inert_label(ui, &s).clicked() {
                    clicked = true;
                  }
                });

                if clicked || row.response().clicked() {
                  match self.selected_id {
                    None => self.selected_id = Some(entry.id),
                    Some(id) => {
                      if id == entry.id {
                        self.selected_id = None;
                      } else {
                        self.selected_id = Some(entry.id);
                      }
                    }
                  }
                }
              });
            }
          });

        TopBottomPanel::bottom(Id::new("modal#open_project.bottom")).show_inside(ui, |ui| {
          ui.add_space(5.0);
          Sides::new().show(
            ui,
            |ui| {
              let button = ImageButton::new(icons::DELETE);
              if ui.add_enabled(self.selected_id.is_some(), button).clicked() {
                if let Some(selected_id) = self.selected_id {
                  if app
                    .project
                    .borrow()
                    .as_ref()
                    .map(|p| p.id == selected_id)
                    .unwrap_or(false)
                  {
                    app.modals.add(AlertModal::new(
                      "You can't delete the currently opened project. Close the project first!"
                        .to_string(),
                    ));
                  } else {
                    let project = manifest.entries.iter().find(|p| p.id == selected_id);
                    if let Some(project) = project {
                      app.modals.add(ConfirmModal::new(
                        "Delete project?".to_string(),
                        format!(
                          "The project {} will be permanently deleted! Are you sure?",
                          project.name
                        ),
                        KsngEvent::ProjectDelete(selected_id),
                      ));
                    }
                  }
                }
              }
            },
            |ui| {
              if ui.button("Cancel").clicked() {
                self.open = false;
              }

              if ui
                .add_enabled(self.selected_id.is_some(), Button::new("Open"))
                .clicked()
              {
                if let Some(selected_id) = self.selected_id {
                  app.dispatch(KsngEvent::ProjectOpenId(selected_id));
                }
                self.open = false;
              }
            },
          );
        });
      });
    });

    if modal.should_close() {
      self.open = false;
    }
  }
}
