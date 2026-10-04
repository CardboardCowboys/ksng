use egui::{Button, Key, KeyboardShortcut, MenuBar, Modifiers, Sides, Ui, WidgetText};
use klib::{audio::info::AudioFileInfo, objects::track::TrackType};

use crate::{
  KsngContext,
  app::AppTabInitializer,
  commands::{event::AddAudioEventCommand, track::AddTrackCommand},
  modals::{alert::AlertModal, open_file::OpenFileModal},
  util::ui_event::KsngEvent,
};
enum MenuEntry {
  MenuItem(MenuItemBuilder),
  Divider,
}

type MenuItemAction = Box<dyn Fn(&KsngContext)>;

struct MenuItemBuilder {
  text: WidgetText,
  shortcut: Option<KeyboardShortcut>,
  action: Option<MenuItemAction>,
  enabled: bool,
  children: Vec<MenuEntry>,
  min_size: Option<f32>,
}

impl MenuItemBuilder {
  pub fn new(
    text: impl Into<WidgetText>,
    action: impl Fn(&KsngContext) + 'static,
  ) -> MenuItemBuilder {
    MenuItemBuilder {
      text: text.into(),
      shortcut: None,
      action: Some(Box::new(action)),
      enabled: true,
      children: Vec::new(),
      min_size: None,
    }
  }

  pub fn submenu(text: impl Into<WidgetText>) -> MenuItemBuilder {
    MenuItemBuilder {
      text: text.into(),
      shortcut: None,
      action: None,
      enabled: true,
      children: Vec::new(),
      min_size: None,
    }
  }

  pub fn enabled(mut self, enabled: bool) -> MenuItemBuilder {
    self.enabled = enabled;
    self
  }

  pub fn shortcut(mut self, key: Key, modifiers: Modifiers) -> MenuItemBuilder {
    self.shortcut = Some(KeyboardShortcut::new(modifiers, key));
    self
  }

  pub fn child(mut self, child: MenuItemBuilder) -> MenuItemBuilder {
    self.children.push(MenuEntry::MenuItem(child));
    self
  }

  pub fn divider(mut self) -> MenuItemBuilder {
    self.children.push(MenuEntry::Divider);
    self
  }

  pub fn min_size(mut self, size: f32) -> MenuItemBuilder {
    self.min_size = Some(size);
    self
  }

  pub fn show(&self, ui: &mut Ui, app: &KsngContext) {
    if let Some(action) = &self.action {
      let mut button = Button::new(self.text.clone());
      if let Some(shortcut) = &self.shortcut {
        button = button.shortcut_text(ui.ctx().format_shortcut(shortcut));
      }

      if ui.add_enabled(self.enabled, button).clicked() {
        (action)(app);
        ui.close();
      }
    } else {
      ui.add_enabled_ui(self.enabled, |ui| {
        ui.menu_button(self.text.clone(), |ui| {
          if let Some(min_size) = self.min_size {
            ui.set_min_width(min_size);
          }
          for child in &self.children {
            match child {
              MenuEntry::MenuItem(item) => {
                item.show(ui, app);
              }
              MenuEntry::Divider => {
                ui.separator();
              }
            }
          }
        });
      });
    }
  }

  pub fn process_hotkeys(&self, ui: &mut Ui, app: &KsngContext) {
    if let Some(action) = &self.action
      && let Some(shortcut) = &self.shortcut
      && ui.input_mut(|i| i.consume_shortcut(shortcut))
    {
      (action)(app);
    } else {
      for child in &self.children {
        if let MenuEntry::MenuItem(item) = &child {
          item.process_hotkeys(ui, app);
        }
      }
    }
  }
}

#[derive(Default)]
struct MenuBuilder {
  children: Vec<MenuItemBuilder>,
}

impl MenuBuilder {
  pub fn item(mut self, child: MenuItemBuilder) -> MenuBuilder {
    self.children.push(child);
    self
  }

  pub fn show(&self, ui: &mut Ui, app: &KsngContext) {
    for child in &self.children {
      child.show(ui, app);
    }
  }

  pub fn process_hotkeys(&self, ui: &mut Ui, app: &KsngContext) {
    for child in &self.children {
      child.process_hotkeys(ui, app);
    }
  }
}

fn build_menu(app: &KsngContext) -> MenuBuilder {
  let project = app.project.borrow();
  let is_dirty = project.as_ref().map(|f| f.dirty).unwrap_or(false);

  let file = MenuItemBuilder::submenu("File")
    .min_size(150.0)
    .child(
      MenuItemBuilder::new("New", |app| {
        app.dispatch_warn_dirty(KsngEvent::ProjectNew);
      })
      .shortcut(Key::N, Modifiers::COMMAND),
    )
    .child(
      MenuItemBuilder::new("Open", |app| {
        app.dispatch_warn_dirty(KsngEvent::ProjectOpen);
      })
      .shortcut(Key::O, Modifiers::COMMAND),
    )
    .child(
      MenuItemBuilder::new("Save", |app| {
        app.dispatch(KsngEvent::ProjectSave);
      })
      .shortcut(Key::S, Modifiers::COMMAND)
      .enabled(is_dirty),
    )
    .child(
      MenuItemBuilder::new("Close", |app| {
        app.dispatch_warn_dirty(KsngEvent::ProjectClose);
      })
      .enabled(project.is_some()),
    )
    .divider()
    .child(
      MenuItemBuilder::submenu("Export")
        .enabled(project.is_some())
        .child(MenuItemBuilder::new("Video...", |app| {
          app.dispatch(KsngEvent::ProjectExportVideo);
        })),
    )
    .divider()
    .child(MenuItemBuilder::new("Quit", |app| {
      app.dispatch_warn_dirty(KsngEvent::Quit);
    }));

  let undo_desc = app.commands.undo_description();
  let undo_label = undo_desc
    .as_ref()
    .map(|d| format!("Undo {d}"))
    .unwrap_or("Undo".to_string());

  let redo_desc = app.commands.redo_description();
  let redo_label = redo_desc
    .as_ref()
    .map(|d| format!("Redo {d}"))
    .unwrap_or("Redo".to_string());

  let edit = MenuItemBuilder::submenu("Edit")
    .min_size(200.0)
    .child(
      MenuItemBuilder::new(undo_label, |app| {
        app.dispatch(KsngEvent::Undo);
      })
      .enabled(undo_desc.is_some())
      .shortcut(Key::Z, Modifiers::COMMAND),
    )
    .child(
      MenuItemBuilder::new(redo_label, |app| {
        app.dispatch(KsngEvent::Redo);
      })
      .enabled(redo_desc.is_some())
      .shortcut(Key::Y, Modifiers::COMMAND),
    )
    .divider()
    .child(MenuItemBuilder::new("Preferences...", |app| {
      app.dispatch(KsngEvent::OpenTabWindow(AppTabInitializer::Preferences));
    }));

  let view = MenuItemBuilder::submenu("View")
    .child(
      MenuItemBuilder::new("Player", |app| {
        app.dispatch(KsngEvent::OpenTabWindow(AppTabInitializer::Player))
      })
      .shortcut(Key::Num1, Modifiers::COMMAND),
    )
    .child(
      MenuItemBuilder::new("Lyrics Editor", |app| {
        app.dispatch(KsngEvent::OpenTabWindow(AppTabInitializer::LyricsEditor))
      })
      .shortcut(Key::Num2, Modifiers::COMMAND),
    )
    .child(
      MenuItemBuilder::new("Timeline", |app| {
        app.dispatch(KsngEvent::OpenTabWindow(AppTabInitializer::Timeline))
      })
      .shortcut(Key::Num3, Modifiers::COMMAND),
    );

  let lyrics_track_id = project.as_ref().and_then(|p| {
    p.file
      .tracks
      .iter()
      .find(|t| t.track_type == TrackType::Lyrics && app.selection.is_track_selected(t.id))
      .map(|t| t.id)
  });

  let track = MenuItemBuilder::submenu("Track")
    .enabled(project.is_some())
    .min_size(150.0)
    .child(
      MenuItemBuilder::submenu("Add")
        .child(MenuItemBuilder::new("Lyrics", |app| {
          app
            .commands
            .dispatch(AddTrackCommand::new(TrackType::Lyrics));
        }))
        .child(MenuItemBuilder::new("Audio", |app| {
          app
            .commands
            .dispatch(AddTrackCommand::new(TrackType::Audio));
        })),
    )
    .divider()
    .child(
      MenuItemBuilder::new("Sync Lyrics...", move |app| {
        app.dispatch(KsngEvent::OpenTabWindow(AppTabInitializer::Sync {
          track_id: lyrics_track_id.unwrap(),
        }));
      })
      .enabled(lyrics_track_id.is_some())
      .shortcut(Key::L, Modifiers::COMMAND),
    );

  let audio_track_id = project.as_ref().and_then(|p| {
    p.file
      .tracks
      .iter()
      .find(|t| t.track_type == TrackType::Audio && app.selection.is_track_selected(t.id))
      .map(|t| t.id)
  });

  let event = MenuItemBuilder::submenu("Event")
    .enabled(project.is_some())
    .min_size(150.0)
    .child(
      MenuItemBuilder::submenu("Add").child(
        MenuItemBuilder::new("Audio", move |app| {
          let id = audio_track_id.unwrap();
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
        })
        .enabled(audio_track_id.is_some()),
      ),
    );

  let models = MenuItemBuilder::submenu("Models")
    .child(MenuItemBuilder::new("Model Manager", |app| {
      app.dispatch(KsngEvent::OpenTabWindow(AppTabInitializer::Models));
    }))
    .divider()
    .child(MenuItemBuilder::new("Stem Separation", |app| {
      app.dispatch(KsngEvent::OpenTabWindow(AppTabInitializer::StemSeparation));
    }));

  let help = MenuItemBuilder::submenu("Help")
    .min_size(150.0)
    .child(MenuItemBuilder::new("Log", |app| {
      app.dispatch(KsngEvent::OpenTabWindow(AppTabInitializer::Log));
    }));

  MenuBuilder::default()
    .item(file)
    .item(edit)
    .item(view)
    .item(track)
    .item(event)
    .item(models)
    .item(help)
}

pub fn menu_bar(app: &KsngContext, ui: &mut Ui) {
  let menu = build_menu(app);

  MenuBar::new().ui(ui, |ui| {
    let project = app.project.borrow();
    Sides::new().show(
      ui,
      |ui| {
        menu.show(ui, app);
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

pub fn process_menu_hotkeys(app: &KsngContext, ui: &mut Ui) {
  build_menu(app).process_hotkeys(ui, app);
}
