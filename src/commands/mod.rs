use std::{
  cell::RefCell,
  collections::{LinkedList, VecDeque},
};

use crate::{error::UiError, KsngApp};

pub mod event;
pub mod track;

pub trait Command {
  fn can_undo(&self) -> bool;
  fn description(&self) -> String;
  fn make_dirty(&self) -> bool {
    true
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
      if command.make_dirty() {
        app.set_dirty_state(true);
      }
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
      if command.make_dirty() {
        app.set_dirty_state(true);
      }
      self.redo_queue.borrow_mut().push_back(command);
    }

    Ok(())
  }

  pub fn redo(&self, app: &KsngApp) -> Result<(), UiError> {
    if let Some(command) = self.redo_queue.borrow_mut().pop_back() {
      command.execute(app)?;
      if command.make_dirty() {
        app.set_dirty_state(true);
      }
      self.undo_queue.borrow_mut().push_back(command);
    }

    Ok(())
  }
}
