use std::{cell::RefCell, collections::HashMap, path::PathBuf};

use klib::{
  audio::info::AudioFileInfo,
  objects::{
    attachment::Attachment,
    audio::{AudioFile, AudioFileSource},
    event::{Event, EventValue},
    track::{EventList, Track},
  },
};
use uuid::Uuid;

use crate::{commands::Command, fs::KsngAttachmentResolver, util::error::UiError};

pub struct HtdemucsResultCommand {
  source_event_id: Uuid,
  result_paths: HashMap<String, PathBuf>,
  created_tracks: RefCell<Vec<Uuid>>,
  created_attachments: RefCell<Vec<Uuid>>,
}

impl HtdemucsResultCommand {
  pub fn new(
    source_event_id: Uuid,
    result_paths: HashMap<String, PathBuf>,
  ) -> HtdemucsResultCommand {
    HtdemucsResultCommand {
      source_event_id,
      result_paths,
      created_tracks: Default::default(),
      created_attachments: Default::default(),
    }
  }
}

impl Command for HtdemucsResultCommand {
  fn can_undo(&self) -> bool {
    true
  }

  fn description(&self) -> String {
    "Stem Separation Results".to_owned()
  }

  fn update_flags(&self) -> super::UpdateFlags {
    super::UpdateFlags::MAKE_DIRTY | super::UpdateFlags::AUDIO_CHANGED
  }

  fn execute(&self, app: &crate::KsngContext) -> Result<(), crate::util::error::UiError> {
    let mut project = app.project.borrow_mut();
    let file = project
      .as_mut()
      .map(|p| &mut p.file)
      .ok_or(UiError::InvalidCommand(
        "HtdemucsResultCommand without project".to_string(),
      ))?;

    let source_event = file
      .tracks
      .iter()
      .flat_map(|t| t.events.find_id(self.source_event_id))
      .next()
      .ok_or(UiError::InvalidCommand(format!(
        "Can't find source event ID {}",
        self.source_event_id
      )))?;
    let offset = match &source_event.value {
      Some(EventValue::AudioClip { offset, .. }) => *offset,
      _ => {
        return Err(UiError::InvalidCommand(format!(
          "Could not get source event offset for ID {}",
          self.source_event_id
        )));
      }
    };
    let (start, end) = (source_event.start_timecode, source_event.end_timecode);

    let mut next_order = file.tracks.iter().map(|t| t.order + 1).max().unwrap_or(0);
    let mut created_tracks = Vec::new();
    let mut created_attachments = Vec::new();
    // stfu clippy
    #[allow(clippy::explicit_counter_loop)]
    for (name, path) in &self.result_paths {
      let mut track = Track::new_audio(next_order);

      let size = std::fs::metadata(path)?.len() as usize;
      let info = AudioFileInfo::from_file(path.as_path())?.ok_or(UiError::Audio(format!(
        "Could not read info of Htdemucs result file {path:?}"
      )))?;
      let attachment = Attachment {
        id: Uuid::new_v4(),
        name: name.clone(),
        mime_type: info.mime_type,
        size,
        source: klib::objects::attachment::AttachmentSource::Managed,
      };

      let audio_file = AudioFile {
        id: Uuid::new_v4(),
        file_type: info.audio_type,
        source: AudioFileSource::Attachment(attachment.id),
      };
      track
        .events
        .insert(Event::new_audio(start, end, offset, audio_file));

      std::fs::copy(
        path.as_path(),
        KsngAttachmentResolver::get_path_for(attachment.id),
      )?;
      created_attachments.push(attachment.id);
      file.attachments.push(attachment);
      created_tracks.push(track.id);
      file.tracks.push(track);
      next_order += 1;
    }

    *self.created_attachments.borrow_mut() = created_attachments;
    *self.created_tracks.borrow_mut() = created_tracks;
    Ok(())
  }

  fn undo(&self, app: &crate::KsngContext) -> Result<(), crate::util::error::UiError> {
    let mut project = app.project.borrow_mut();
    let file = project
      .as_mut()
      .map(|p| &mut p.file)
      .ok_or(UiError::InvalidCommand(
        "HtdemucsResultCommand without project".to_string(),
      ))?;

    let created_tracks = &*self.created_tracks.borrow();
    let created_attachments = &*self.created_attachments.borrow();

    for attachment_id in created_attachments {
      let path = KsngAttachmentResolver::get_path_for(*attachment_id);
      if std::fs::exists(&path)? {
        std::fs::remove_file(&path)?;
      }
    }

    file.tracks.retain(|t| !created_tracks.contains(&t.id));
    file
      .attachments
      .retain(|t| !created_attachments.contains(&t.id));

    Ok(())
  }
}
