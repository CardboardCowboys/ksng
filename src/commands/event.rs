use std::{cell::RefCell, path::PathBuf};

use klib::{
  objects::{
    audio::{AudioFile, AudioFileSource},
    event::Event,
    track::EventList,
  },
  timecode::Timecode,
};
use uuid::Uuid;

use crate::{
  KsngApp,
  audio::info::AudioFileInfo,
  commands::{Command, UpdateFlags},
  util::error::UiError,
};

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

  fn update_flags(&self) -> UpdateFlags {
    UpdateFlags::MAKE_DIRTY | UpdateFlags::AUDIO_CHANGED
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

pub struct SetEventTimingsCommand {
  title: String,
  event_ids: Vec<Uuid>,
  new_timings: Vec<(Timecode, Timecode)>,
  old_timings: RefCell<Vec<(Timecode, Timecode)>>,
}

impl SetEventTimingsCommand {
  pub fn new(event_ids: &[Uuid], timings: &[(Timecode, Timecode)]) -> SetEventTimingsCommand {
    Self::new_with_title(
      if event_ids.len() == 1 {
        "Set event timing".to_string()
      } else {
        "Set event timings".to_string()
      },
      event_ids,
      timings,
    )
  }

  pub fn new_with_title(
    title: String,
    event_ids: &[Uuid],
    timings: &[(Timecode, Timecode)],
  ) -> SetEventTimingsCommand {
    SetEventTimingsCommand {
      title,
      event_ids: event_ids.into(),
      new_timings: timings.into(),
      old_timings: Default::default(),
    }
  }
}

impl Command for SetEventTimingsCommand {
  fn can_undo(&self) -> bool {
    true
  }

  fn description(&self) -> String {
    self.title.clone()
  }

  fn update_flags(&self) -> UpdateFlags {
    UpdateFlags::MAKE_DIRTY
      | UpdateFlags::INVALIDATE_VIDEO
      | UpdateFlags::LYRICS_CHANGED
      | UpdateFlags::AUDIO_CHANGED
  }

  fn execute(&self, app: &KsngApp) -> Result<(), UiError> {
    let mut project = app.project.borrow_mut();
    let file = project
      .as_mut()
      .map(|p| &mut p.file)
      .ok_or(UiError::InvalidCommand(
        "SetEventTimings without project".to_string(),
      ))?;

    if self.event_ids.len() != self.new_timings.len() {
      return Err(UiError::InvalidCommand(
        "event_ids must be the same len as new_timings in SetEventTimings".to_string(),
      ));
    }

    let mut old_timings = Vec::new();

    for (i, id) in self.event_ids.iter().enumerate() {
      for track in &mut file.tracks {
        if let Some(mut event) = track.events.take_id(*id) {
          let (new_start, new_end) = self.new_timings[i];
          old_timings.push((event.start_timecode, event.end_timecode));
          event.start_timecode = new_start;
          event.end_timecode = new_end;
          track.events.insert(event);
          break;
        }
      }
    }

    *self.old_timings.borrow_mut() = old_timings;

    Ok(())
  }

  fn undo(&self, app: &KsngApp) -> Result<(), UiError> {
    let mut project = app.project.borrow_mut();
    let file = project
      .as_mut()
      .map(|p| &mut p.file)
      .ok_or(UiError::InvalidCommand(
        "SetEventTimings without project".to_string(),
      ))?;

    let old_timings = self.old_timings.borrow();

    if self.event_ids.len() != old_timings.len() {
      return Err(UiError::InvalidCommand(
        "event_ids must be the same len as old_timings in SetEventTimings".to_string(),
      ));
    }

    for (i, id) in self.event_ids.iter().enumerate() {
      for track in &mut file.tracks {
        if let Some(mut event) = track.events.take_id(*id) {
          let (new_start, new_end) = old_timings[i];
          event.start_timecode = new_start;
          event.end_timecode = new_end;
          track.events.insert(event);
          break;
        }
      }
    }

    Ok(())
  }
}
