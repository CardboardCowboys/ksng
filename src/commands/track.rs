use std::cell::RefCell;

use klib::objects::track::{Track, TrackType, TrackValue};
use uuid::Uuid;

use crate::{
  KsngApp,
  commands::{Command, UpdateFlags},
  util::{error::UiError, ui_event::KsngEvent},
};

pub struct AddTrackCommand {
  track_type: TrackType,
  added_track_id: RefCell<Option<Uuid>>,
}

impl AddTrackCommand {
  pub fn new(track_type: TrackType) -> AddTrackCommand {
    AddTrackCommand {
      track_type,
      added_track_id: Default::default(),
    }
  }
}

impl Command for AddTrackCommand {
  fn can_undo(&self) -> bool {
    true
  }

  fn description(&self) -> String {
    format!("Add {:?} Track", self.track_type)
  }

  fn update_flags(&self) -> UpdateFlags {
    UpdateFlags::MAKE_DIRTY | UpdateFlags::INVALIDATE_VIDEO
  }

  fn execute(&self, app: &KsngApp) -> Result<(), UiError> {
    let mut project = app.project.borrow_mut();
    let file = project
      .as_mut()
      .map(|p| &mut p.file)
      .ok_or(UiError::InvalidCommand(
        "AddTrackCommand without project".to_string(),
      ))?;

    let next_order = file.tracks.iter().map(|t| t.order + 1).max().unwrap_or(0);
    let new_track = Track::new_type(self.track_type, next_order);
    self.added_track_id.borrow_mut().replace(new_track.id);
    file.tracks.push(new_track);

    Ok(())
  }

  fn undo(&self, app: &KsngApp) -> Result<(), UiError> {
    let mut project = app.project.borrow_mut();
    let file = project
      .as_mut()
      .map(|p| &mut p.file)
      .ok_or(UiError::InvalidCommand(
        "AddTrackCommand without project".to_string(),
      ))?;

    if let Some(added_id) = *self.added_track_id.borrow() {
      app.selection.remove_track(added_id);
      file.tracks.retain(|t| t.id != added_id);
    }

    Ok(())
  }
}

/// Sets the mute state of an audio track.
pub struct MuteTrackCommand {
  track_id: Uuid,
  to_mute_state: bool,
}

impl MuteTrackCommand {
  pub fn new(track: &Track) -> MuteTrackCommand {
    let mute_state = if let Some(TrackValue::Audio(audio_value)) = &track.track_value {
      audio_value.muted
    } else {
      false
    };

    MuteTrackCommand {
      track_id: track.id,
      to_mute_state: !mute_state,
    }
  }
}

impl Command for MuteTrackCommand {
  fn can_undo(&self) -> bool {
    true
  }

  fn description(&self) -> String {
    if self.to_mute_state {
      "Mute Audio Track"
    } else {
      "Unmute Audio Track"
    }
    .to_string()
  }

  fn execute(&self, app: &KsngApp) -> Result<(), UiError> {
    let mut project = app.project.borrow_mut();
    let file = project
      .as_mut()
      .map(|p| &mut p.file)
      .ok_or(UiError::InvalidCommand(
        "MuteTrackCommand without project".to_string(),
      ))?;

    let track = file
      .tracks
      .iter_mut()
      .find(|t| t.id == self.track_id)
      .ok_or(UiError::InvalidCommand(
        "MuteTrackCommand can't find target track".to_string(),
      ))?;

    if let Some(TrackValue::Audio(audio_value)) = &mut track.track_value {
      audio_value.muted = self.to_mute_state;
    }

    app.dispatch(KsngEvent::AudioChanged);

    Ok(())
  }

  fn undo(&self, app: &KsngApp) -> Result<(), UiError> {
    let mut project = app.project.borrow_mut();
    let file = project
      .as_mut()
      .map(|p| &mut p.file)
      .ok_or(UiError::InvalidCommand(
        "MuteTrackCommand without project".to_string(),
      ))?;

    let track = file
      .tracks
      .iter_mut()
      .find(|t| t.id == self.track_id)
      .ok_or(UiError::InvalidCommand(
        "MuteTrackCommand can't find target track".to_string(),
      ))?;

    if let Some(TrackValue::Audio(audio_value)) = &mut track.track_value {
      audio_value.muted = !self.to_mute_state;
    }

    app.dispatch(KsngEvent::AudioChanged);

    Ok(())
  }
}

pub struct EditTrackConfigCommand {
  track_id: Uuid,
  prev_config: RefCell<Option<TrackValue>>,
  new_config: TrackValue,
}

impl EditTrackConfigCommand {
  pub fn new(track_id: Uuid, new_config: TrackValue) -> Self {
    EditTrackConfigCommand {
      track_id,
      new_config,
      prev_config: RefCell::new(None),
    }
  }
}

impl Command for EditTrackConfigCommand {
  fn can_undo(&self) -> bool {
    true
  }

  fn update_flags(&self) -> UpdateFlags {
    UpdateFlags::MAKE_DIRTY | UpdateFlags::INVALIDATE_VIDEO
  }

  fn description(&self) -> String {
    match &self.new_config {
      TrackValue::Audio(..) => "Edit Audio Track Config".to_string(),
      TrackValue::Lyrics(..) => "Edit Lyrics Track Config".to_string(),
    }
  }

  fn execute(&self, app: &KsngApp) -> Result<(), UiError> {
    let mut project = app.project.borrow_mut();
    let file = project
      .as_mut()
      .map(|p| &mut p.file)
      .ok_or(UiError::InvalidCommand(
        "EditTrackConfigCommand without project".to_string(),
      ))?;

    let track = file
      .tracks
      .iter_mut()
      .find(|t| t.id == self.track_id)
      .ok_or(UiError::InvalidCommand(
        "EditTrackConfigCommand can't find target track".to_string(),
      ))?;

    self.prev_config.replace(track.track_value.clone());
    track.track_value = Some(self.new_config.clone());

    Ok(())
  }

  fn undo(&self, app: &KsngApp) -> Result<(), UiError> {
    let mut project = app.project.borrow_mut();
    let file = project
      .as_mut()
      .map(|p| &mut p.file)
      .ok_or(UiError::InvalidCommand(
        "EditTrackConfigCommand without project".to_string(),
      ))?;

    let track = file
      .tracks
      .iter_mut()
      .find(|t| t.id == self.track_id)
      .ok_or(UiError::InvalidCommand(
        "EditTrackConfigCommand can't find target track".to_string(),
      ))?;

    track.track_value = self.prev_config.borrow().clone();

    Ok(())
  }
}
