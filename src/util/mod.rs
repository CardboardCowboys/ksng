use itertools::Itertools;
use klib::objects::file::File;
use uuid::Uuid;

pub mod r#async;
pub mod error;
pub mod logger;
pub mod ui;
pub mod ui_event;

pub fn calculate_track_name(file: &File, track_id: Uuid) -> String {
  let Some(track) = file.tracks.iter().find(|t| t.id == track_id) else {
    return "Invalid Track".to_string();
  };

  let mut n_of_type = 0;
  for t in file.tracks.iter().sorted_by_key(|t| t.order) {
    if t.track_type == track.track_type {
      n_of_type += 1;
    }

    if t.id == track_id {
      break;
    }
  }

  format!("{:?} Track #{n_of_type}", track.track_type)
}
