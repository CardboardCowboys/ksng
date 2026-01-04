use std::hash::{DefaultHasher, Hash, Hasher};

use egui::{Button, Sides, Window};
use klib::objects::track::{AudioTrackValue, LyricsTrackValue, TrackType, TrackValue};
use uuid::Uuid;

use crate::{
  commands::track::EditTrackConfigCommand, components::config_editor::config_editor,
  windows::KWindow,
};

pub struct TrackConfigWindow {
  open: bool,
  track_id: Uuid,
  new_value: TrackValue,
  dirty: bool,
  unique_value: u64,
  should_request_focus: bool,
}

impl TrackConfigWindow {
  pub fn new(track: &klib::objects::track::Track) -> Self {
    let mut hasher = DefaultHasher::new();
    "TrackConfigEditor".hash(&mut hasher);
    track.id.hash(&mut hasher);

    let track_value = track.track_value.clone().unwrap_or(match track.track_type {
      TrackType::Lyrics => TrackValue::Lyrics(LyricsTrackValue::default()),
      TrackType::Audio => TrackValue::Audio(AudioTrackValue::default()),
    });

    TrackConfigWindow {
      open: true,
      track_id: track.id,
      new_value: track_value,
      dirty: false,
      unique_value: hasher.finish(),
      should_request_focus: false,
    }
  }
}

impl KWindow for TrackConfigWindow {
  fn should_cleanup(&self) -> bool {
    !self.open
  }

  fn process(&mut self, app: &crate::KsngApp, context: &egui::Context) {
    if !self.open {
      return;
    }

    let track_idx = app
      .project
      .borrow()
      .iter()
      .flat_map(|p| p.file.tracks.iter().position(|t| t.id == self.track_id))
      .next();

    let Some(track_idx) = track_idx else {
      // if there's no track_idx we have no track to work on (perhaps deleted or addition undone)
      self.open = false;
      return;
    };

    let title = format!("Editing Track {track_idx} Config");

    let window = Window::new(&title).show(context, |ui| {
      ui.set_width(250.0);

      match &mut self.new_value {
        TrackValue::Audio(audio_track_value) => {
          if config_editor(ui, format!("{}_editor", title), audio_track_value) {
            self.dirty = true;
          }
        }
        TrackValue::Lyrics(lyrics_track_value) => {
          if config_editor(ui, format!("{}_editor", title), lyrics_track_value) {
            self.dirty = true;
          }
        }
      };

      Sides::new().show(
        ui,
        |_ui| {},
        |ui| {
          if ui.button("Cancel").clicked() {
            self.open = false;
          }

          if ui.add_enabled(self.dirty, Button::new("Apply")).clicked() {
            self.dirty = false;
            app.commands.dispatch(EditTrackConfigCommand::new(
              self.track_id,
              self.new_value.clone(),
            ));
          }

          if ui.add_enabled(self.dirty, Button::new("OK")).clicked() {
            self.dirty = false;
            app.commands.dispatch(EditTrackConfigCommand::new(
              self.track_id,
              self.new_value.clone(),
            ));
            self.open = false;
          }
        },
      )
    });

    if let Some(window) = window {
      if self.should_request_focus {
        window.response.request_focus();
        self.should_request_focus = false;
      }
    }
  }

  fn request_focus(&mut self) {
    self.should_request_focus = true;
  }

  fn unique_value(&self) -> Option<u64> {
    Some(self.unique_value)
  }
}
