use std::{cell::RefCell, collections::HashSet};

use uuid::Uuid;

#[derive(Default)]
pub struct SelectionManager {
  selected_tracks: RefCell<HashSet<Uuid>>,
  selected_events: RefCell<HashSet<Uuid>>,
}

impl SelectionManager {
  pub fn select_track(&self, id: Uuid, single: bool) {
    self.select(
      id,
      single,
      &mut self.selected_tracks.borrow_mut(),
      &mut self.selected_events.borrow_mut(),
    )
  }

  pub fn select_event(&self, id: Uuid, single: bool) {
    self.select(
      id,
      single,
      &mut self.selected_events.borrow_mut(),
      &mut self.selected_tracks.borrow_mut(),
    )
  }

  fn select(
    &self,
    id: Uuid,
    single: bool,
    selected: &mut HashSet<Uuid>,
    other: &mut HashSet<Uuid>,
  ) {
    if selected.contains(&id) {
      if single {
        let was_multiple = selected.len() > 1;
        selected.clear();
        if was_multiple {
          selected.insert(id);
        }
      } else {
        selected.remove(&id);
      }
    } else {
      if single {
        selected.clear();
      }
      selected.insert(id);
      other.clear();
    }
  }

  pub fn is_track_selected(&self, track_id: Uuid) -> bool {
    self.selected_tracks.borrow().contains(&track_id)
  }

  pub fn is_event_selected(&self, event_id: Uuid) -> bool {
    self.selected_events.borrow().contains(&event_id)
  }

  pub fn selected_tracks(&self) -> Vec<Uuid> {
    self.selected_tracks.borrow().iter().copied().collect()
  }

  pub fn selected_events(&self) -> Vec<Uuid> {
    self.selected_events.borrow().iter().copied().collect()
  }

  pub fn clear(&self) {
    self.selected_tracks.borrow_mut().clear();
    self.selected_events.borrow_mut().clear();
  }

  pub fn clear_events(&self) {
    self.selected_events.borrow_mut().clear();
  }

  pub fn clear_tracks(&self) {
    self.selected_tracks.borrow_mut().clear();
  }

  pub fn remove_track(&self, id: Uuid) {
    self.selected_tracks.borrow_mut().remove(&id);
  }

  pub fn remove_event(&self, id: Uuid) {
    self.selected_events.borrow_mut().remove(&id);
  }
}
