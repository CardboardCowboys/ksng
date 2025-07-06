use egui::{Button, Context, Sides, Ui};
use klib::objects::track::TrackType;

use crate::{commands::track::AddTrackCommand, ui_event::KsngEvent, KsngApp};

pub fn menu_bar(app: &KsngApp, ctx: &Context, ui: &mut Ui) {
  egui::menu::bar(ui, |ui| {
    let project = app.project.borrow();
    Sides::new().show(
      ui,
      |ui| {
        let is_web = cfg!(target_arch = "wasm32");
        ui.menu_button("File", |ui| {
          if ui.button("New").clicked() {
            app.dispatch_warn_dirty(KsngEvent::ProjectNew);
          }

          if ui.button("Open").clicked() {
            app.dispatch_warn_dirty(KsngEvent::ProjectOpen);
          }

          let is_dirty = project.as_ref().map(|f| f.dirty).unwrap_or(false);
          if ui.add_enabled(is_dirty, Button::new("Save")).clicked() {
            app.dispatch(KsngEvent::ProjectSave);
          }

          if ui
            .add_enabled(project.is_some(), Button::new("Close"))
            .clicked()
          {
            app.dispatch_warn_dirty(KsngEvent::ProjectClose);
          }

          // NOTE: no File->Quit on web pages!
          if !is_web && ui.button("Quit").clicked() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
          }
        });

        ui.menu_button("Edit", |ui| {
          let undo_desc = app.commands.undo_description();
          let undo_label = undo_desc
            .as_ref()
            .map(|d| format!("Undo {d}"))
            .unwrap_or("Undo".to_string());
          if ui
            .add_enabled(undo_desc.is_some(), Button::new(&undo_label))
            .clicked()
          {
            app.logger.wrap(app.commands.undo(app));
            ui.close_menu();
          }

          let redo_desc = app.commands.redo_description();
          let redo_label = redo_desc
            .as_ref()
            .map(|d| format!("Redo {d}"))
            .unwrap_or("Redo".to_string());
          if ui
            .add_enabled(redo_desc.is_some(), Button::new(&redo_label))
            .clicked()
          {
            app.logger.wrap(app.commands.redo(app));
            ui.close_menu();
          }
        });

        ui.add_enabled_ui(project.is_some(), |ui| {
          ui.menu_button("Track", |ui| {
            ui.menu_button("Add", |ui| {
              if ui.button("Lyrics").clicked() {
                app
                  .commands
                  .dispatch(AddTrackCommand::new(TrackType::Lyrics));
                ui.close_menu();
              }

              if ui.button("Audio").clicked() {
                app
                  .commands
                  .dispatch(AddTrackCommand::new(TrackType::Audio));
                ui.close_menu();
              }
            });
          });
        });
      },
      |ui| {
        if let Some(project) = project.as_ref() {
          ui.label(
            format!(
              "Project: {}",
              project.name.as_ref().unwrap_or(&"(unnamed)".to_string())
            ) + match project.dirty {
              true => "*",
              false => "",
            },
          );
        } else {
          ui.label("No project");
        }
      },
    )
  });
}
