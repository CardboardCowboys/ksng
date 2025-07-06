use egui::{Button, Context, Sides, Ui};
use klib::objects::track::TrackType;

use crate::{
  audio::AudioFileInfo,
  commands::{event::AddAudioEventCommand, track::AddTrackCommand},
  modals::{alert::AlertModal, open_file::OpenFileModal},
  ui_event::KsngEvent,
  KsngApp,
};

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
            ui.close_menu();
          }

          if ui.button("Open").clicked() {
            app.dispatch_warn_dirty(KsngEvent::ProjectOpen);
            ui.close_menu();
          }

          let is_dirty = project.as_ref().map(|f| f.dirty).unwrap_or(false);
          if ui.add_enabled(is_dirty, Button::new("Save")).clicked() {
            app.dispatch(KsngEvent::ProjectSave);
            ui.close_menu();
          }

          if ui
            .add_enabled(project.is_some(), Button::new("Close"))
            .clicked()
          {
            app.dispatch_warn_dirty(KsngEvent::ProjectClose);
            ui.close_menu();
          }

          // NOTE: no File->Quit on web pages!
          if !is_web && ui.button("Quit").clicked() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            ui.close_menu();
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
            app.dispatch(KsngEvent::Undo);
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
            app.dispatch(KsngEvent::Redo);
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

          ui.menu_button("Event", |ui| {
            ui.add_enabled_ui(app.selection.selected_tracks().len() == 1, |ui| {
              ui.menu_button("Add", |ui| {
                let audio_track = project.as_ref().and_then(|p| {
                  p.file.tracks.iter().find(|t| {
                    t.track_type == TrackType::Audio && app.selection.is_track_selected(t.id)
                  })
                });

                if ui
                  .add_enabled(audio_track.is_some(), Button::new("Audio"))
                  .clicked()
                {
                  let id = audio_track.unwrap().id;
                  app.modals.add(OpenFileModal::new(
                    "Audio Files".to_string(),
                    vec!["mp3", "wav", "flac", "aac", "ogg", "opus"],
                    move |app, path| {
                      if let Some(info) = app.logger.wrap(AudioFileInfo::from_file(&path)) {
                        match info {
                          Some(info) => {
                            app
                              .commands
                              .dispatch(AddAudioEventCommand::new(id, path, info));
                          }
                          None => {
                            app.modals.add(AlertModal::new(format!(
                              "Unable to read file {path:?} or unsupported format."
                            )));
                          }
                        }
                      }
                    },
                  ));
                  ui.close_menu();
                }
              });
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
