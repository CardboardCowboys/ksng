use egui::{Button, Sides, Ui};
use klib::objects::track::{AudioTrackValue, LyricsTrackValue, Track, TrackType, TrackValue};
use uuid::Uuid;

use crate::{
  KsngContext, commands::track::EditTrackConfigCommand, components::config_editor::config_editor,
};

#[derive(Clone, Debug)]
pub struct TrackConfigWindow {
  track_id: Uuid,
  new_value: TrackValue,
  dirty: bool,
}

impl PartialEq for TrackConfigWindow {
  fn eq(&self, other: &Self) -> bool {
    self.track_id == other.track_id
  }
}

fn default_track_value(track: &Track) -> TrackValue {
  match track.track_type {
    TrackType::Lyrics => TrackValue::Lyrics(LyricsTrackValue::default()),
    TrackType::Audio => TrackValue::Audio(AudioTrackValue::default()),
  }
}

impl TrackConfigWindow {
  pub fn new(track: &klib::objects::track::Track) -> Self {
    let track_value = track
      .track_value
      .clone()
      .unwrap_or(default_track_value(track));

    TrackConfigWindow {
      track_id: track.id,
      new_value: track_value,
      dirty: false,
    }
  }

  pub const fn track_id(&self) -> Uuid {
    self.track_id
  }

  pub fn show(&mut self, ui: &mut Ui, app: &KsngContext) {
    let project_ref = app.project.borrow();
    let Some(track) = project_ref
      .as_ref()
      .and_then(|p| p.file.tracks.iter().find(|t| t.id == self.track_id))
    else {
      return;
    };

    match &mut self.new_value {
      TrackValue::Audio(audio_track_value) => {
        if config_editor(
          ui,
          format!("track_{}_editor", self.track_id),
          audio_track_value,
        ) {
          self.dirty = true;
        }
      }
      TrackValue::Lyrics(lyrics_track_value) => {
        if config_editor(
          ui,
          format!("track_{}_editor", self.track_id),
          lyrics_track_value,
        ) {
          self.dirty = true;
        }
      }
    };

    Sides::new().show(
      ui,
      |_ui| {},
      |ui| {
        if ui.add_enabled(self.dirty, Button::new("Reset")).clicked() {
          self.new_value = track
            .track_value
            .clone()
            .unwrap_or(default_track_value(track));
          self.dirty = false;
        }

        let is_audio = matches!(self.new_value, TrackValue::Audio(..));

        if ui.add_enabled(self.dirty, Button::new("Apply")).clicked() {
          self.dirty = false;
          app.commands.dispatch(EditTrackConfigCommand::new(
            self.track_id,
            self.new_value.clone(),
            is_audio,
          ));
        }
      },
    );
  }
}
