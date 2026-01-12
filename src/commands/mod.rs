use std::{
  cell::RefCell,
  collections::{LinkedList, VecDeque},
  ops::BitOr,
};

use crate::{KsngApp, util::error::UiError};

pub mod event;
pub mod track;

#[derive(Copy, Clone)]
pub struct UpdateFlags(u32);

impl BitOr for UpdateFlags {
  type Output = UpdateFlags;

  fn bitor(self, rhs: Self) -> Self::Output {
    UpdateFlags(self.0 | rhs.0)
  }
}

impl UpdateFlags {
  pub const MAKE_DIRTY: UpdateFlags = UpdateFlags(1 << 0);
  pub const INVALIDATE_VIDEO: UpdateFlags = UpdateFlags(1 << 1);
  pub const AUDIO_CHANGED: UpdateFlags = UpdateFlags(1 << 2);
  pub const LYRICS_CHANGED: UpdateFlags = UpdateFlags(1 << 3);

  pub fn has_flag(&self, flag: UpdateFlags) -> bool {
    (self.0 & flag.0) == flag.0
  }
}

pub trait Command {
  fn can_undo(&self) -> bool;
  fn description(&self) -> String;
  fn update_flags(&self) -> UpdateFlags {
    UpdateFlags::MAKE_DIRTY
  }

  fn execute(&self, app: &KsngApp) -> Result<(), UiError>;
  fn undo(&self, _app: &KsngApp) -> Result<(), UiError> {
    Ok(())
  }
}

#[derive(Default)]
pub struct CommandDispatcher {
  queue: RefCell<VecDeque<Box<dyn Command>>>,
  undo_queue: RefCell<LinkedList<Box<dyn Command>>>,
  redo_queue: RefCell<LinkedList<Box<dyn Command>>>,
}

impl CommandDispatcher {
  pub fn dispatch<T>(&self, command: T)
  where
    T: Command + 'static,
  {
    self.queue.borrow_mut().push_back(Box::new(command));
  }

  pub fn process(&self, app: &KsngApp) -> Result<(), UiError> {
    let mut queue = self.queue.borrow_mut();
    let mut undo_queue = self.undo_queue.borrow_mut();
    let mut did_command = false;
    while let Some(command) = queue.pop_front() {
      command.execute(app)?;
      Self::apply_flags(command.update_flags(), app);
      if command.can_undo() {
        undo_queue.push_back(command);
      }

      did_command = true;
    }

    if did_command {
      // We performed a command, can't redo from this point.
      self.redo_queue.borrow_mut().clear();
    }

    Ok(())
  }

  pub fn undo_description(&self) -> Option<String> {
    self
      .undo_queue
      .borrow()
      .back()
      .map(|u| u.description().clone())
  }

  pub fn redo_description(&self) -> Option<String> {
    self.redo_queue.borrow().back().map(|u| u.description())
  }

  pub fn undo(&self, app: &KsngApp) -> Result<(), UiError> {
    if let Some(command) = self.undo_queue.borrow_mut().pop_back() {
      command.undo(app)?;
      Self::apply_flags(command.update_flags(), app);
      self.redo_queue.borrow_mut().push_back(command);
    }

    Ok(())
  }

  pub fn redo(&self, app: &KsngApp) -> Result<(), UiError> {
    if let Some(command) = self.redo_queue.borrow_mut().pop_back() {
      command.execute(app)?;
      Self::apply_flags(command.update_flags(), app);
      self.undo_queue.borrow_mut().push_back(command);
    }

    Ok(())
  }

  fn apply_flags(flags: UpdateFlags, app: &KsngApp) {
    if flags.has_flag(UpdateFlags::MAKE_DIRTY) {
      app.set_dirty_state(true);

      if let Some(project) = app.project.borrow_mut().as_mut() {
        project.length = project.file.calculate_length();
      }
    }
    if flags.has_flag(UpdateFlags::INVALIDATE_VIDEO) {
      if let Some(project) = app.project.borrow().as_ref() {
        app.video.borrow_mut().update_from_file(&project.file);
      } else {
        app.video.borrow_mut().clear();
      }
    }
    if flags.has_flag(UpdateFlags::AUDIO_CHANGED) {
      app.playback.borrow_mut().on_audio_change(app);
    }
    if flags.has_flag(UpdateFlags::LYRICS_CHANGED) {
      app.lyrics_editor.borrow_mut().on_lyrics_change(app);
    }
  }
}
