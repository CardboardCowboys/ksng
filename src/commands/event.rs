use std::{
  cell::RefCell,
  path::{Path, PathBuf},
};

use klib::{
  objects::{
    audio::{AudioFile, AudioFileSource},
    event::Event,
    track::{self, EventList},
  },
  timecode::Timecode,
};
use uuid::Uuid;

use crate::{audio::AudioFileInfo, commands::Command, error::UiError, KsngApp};

pub struct AddAudioEventCommand {
  audio_track_id: Uuid,
  audio_file: PathBuf,
  audio_info: AudioFileInfo,
  added_event_id: RefCell<Option<Uuid>>,
}

impl AddAudioEventCommand {
  pub fn new(track_id: Uuid, path: PathBuf, info: AudioFileInfo) -> AddAudioEventCommand {
    AddAudioEventCommand {
      audio_track_id: track_id,
      audio_file: path,
      audio_info: info,
      added_event_id: Default::default(),
    }
  }
}

impl Command for AddAudioEventCommand {
  fn can_undo(&self) -> bool {
    true
  }

  fn description(&self) -> String {
    "Add Audio Event".to_string()
  }

  fn execute(&self, app: &KsngApp) -> Result<(), UiError> {
    let mut project = app.project.borrow_mut();
    let file = project
      .as_mut()
      .map(|p| &mut p.file)
      .ok_or(UiError::InvalidCommand(
        "AddAudioEvent without project".to_string(),
      ))?;

    let track = file
      .tracks
      .iter_mut()
      .find(|t| t.id == self.audio_track_id)
      .ok_or(UiError::InvalidCommand(
        "AddAudioEvent with invalid selected track".to_string(),
      ))?;

    let audio_file = AudioFile {
      id: Uuid::new_v4(),
      file_type: self.audio_info.audio_type,
      source: AudioFileSource::Path(self.audio_file.clone()),
    };

    let event = Event::new_audio(Timecode(0), self.audio_info.length, Timecode(0), audio_file);
    let event_id = event.id;
    track.events.insert(event);

    (*self.added_event_id.borrow_mut()) = Some(event_id);

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

    let track = file
      .tracks
      .iter_mut()
      .find(|t| t.id == self.audio_track_id)
      .ok_or(UiError::InvalidCommand(
        "AddAudioEvent with invalid selected track".to_string(),
      ))?;

    if let Some(added_event_id) = *self.added_event_id.borrow() {
      app.selection.remove_event(added_event_id);
      track.events.remove_id(added_event_id);
    }

    Ok(())
  }
}
