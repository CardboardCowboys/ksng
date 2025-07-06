use std::cell::RefCell;

use klib::objects::track::{Track, TrackType};
use uuid::Uuid;

use crate::{commands::Command, error::UiError, KsngApp};

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
