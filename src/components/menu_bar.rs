use egui::{Button, Context, Key, MenuBar, Modifiers, Sides, Ui};
use klib::objects::track::TrackType;

use crate::{
  KsngApp,
  audio::info::AudioFileInfo,
  commands::{event::AddAudioEventCommand, track::AddTrackCommand},
  modals::{alert::AlertModal, open_file::OpenFileModal},
  util::ui_event::KsngEvent,
  windows::preferences::PreferencesWindow,
};

fn button_with_shortcut(
  ui: &mut Ui,
  text: impl Into<String>,
  key: Key,
  modifiers: Modifiers,
) -> bool {
  button_enabled_with_shortcut(ui, true, text, key, modifiers)
}

fn button_enabled_with_shortcut(
  ui: &mut Ui,
  enabled: bool,
  text: impl Into<String>,
  key: Key,
  modifiers: Modifiers,
) -> bool {
  let mut s = String::new();
  if modifiers.command {
    if cfg!(target_os = "macos") {
      s += "Cmd+";
    } else {
      s += "Ctrl+";
    }
  }
  if modifiers.alt {
    s += "Alt+";
  }
  if modifiers.shift {
    s += "Shift+";
  }
  s += key.name();

  let clicked = ui
    .add_enabled(enabled, Button::new(text.into()).shortcut_text(s))
    .clicked();
  if clicked {
    return true;
  }

  ui.input_mut(|input| input.consume_key(modifiers, key))
}

pub fn menu_bar(app: &KsngApp, ctx: &Context, ui: &mut Ui) {
  MenuBar::new().ui(ui, |ui| {
    let project = app.project.borrow();
    Sides::new().show(
      ui,
      |ui| {
        let is_web = cfg!(target_arch = "wasm32");
        ui.menu_button("File", |ui| {
          ui.set_min_width(150.0);
          if button_with_shortcut(ui, "New", Key::N, Modifiers::COMMAND) {
            app.dispatch_warn_dirty(KsngEvent::ProjectNew);
            ui.close();
          }

          if button_with_shortcut(ui, "Open", Key::O, Modifiers::COMMAND) {
            app.dispatch_warn_dirty(KsngEvent::ProjectOpen);
            ui.close();
          }

          let is_dirty = project.as_ref().map(|f| f.dirty).unwrap_or(false);
          if button_enabled_with_shortcut(ui, is_dirty, "Save", Key::S, Modifiers::COMMAND) {
            app.dispatch(KsngEvent::ProjectSave);
            ui.close();
          }

          if ui
            .add_enabled(project.is_some(), Button::new("Close"))
            .clicked()
          {
            app.dispatch_warn_dirty(KsngEvent::ProjectClose);
            ui.close();
          }

          if !is_web && button_with_shortcut(ui, "Quit", Key::Q, Modifiers::COMMAND) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            ui.close();
          }
        });

        ui.menu_button("Edit", |ui| {
          ui.set_min_width(200.0);
          let undo_desc = app.commands.undo_description();
          let undo_label = undo_desc
            .as_ref()
            .map(|d| format!("Undo {d}"))
            .unwrap_or("Undo".to_string());
          if button_enabled_with_shortcut(
            ui,
            undo_desc.is_some(),
            undo_label,
            Key::Z,
            Modifiers::COMMAND,
          ) {
            app.dispatch(KsngEvent::Undo);
            ui.close();
          }

          let redo_desc = app.commands.redo_description();
          let redo_label = redo_desc
            .as_ref()
            .map(|d| format!("Redo {d}"))
            .unwrap_or("Redo".to_string());
          if button_enabled_with_shortcut(
            ui,
            redo_desc.is_some(),
            redo_label,
            Key::Y,
            Modifiers::COMMAND,
          ) {
            app.dispatch(KsngEvent::Redo);
            ui.close();
          }

          ui.separator();
          if button_with_shortcut(ui, "Preferences...", Key::P, Modifiers::CTRL) {
            app
              .windows
              .add(PreferencesWindow::new(app.preferences.borrow().clone()));
            ui.close();
          }
        });

        ui.add_enabled_ui(project.is_some(), |ui| {
          ui.menu_button("Track", |ui| {
            ui.set_min_width(150.0);
            ui.menu_button("Add", |ui| {
              if ui.button("Lyrics").clicked() {
                app
                  .commands
                  .dispatch(AddTrackCommand::new(TrackType::Lyrics));
                ui.close();
              }

              if ui.button("Audio").clicked() {
                app
                  .commands
                  .dispatch(AddTrackCommand::new(TrackType::Audio));
                ui.close();
              }
            });
          });

          ui.menu_button("Event", |ui| {
            ui.set_min_width(150.0);
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
                  ui.close();
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
