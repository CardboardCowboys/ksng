use klib::{
  objects::{file::File, track::Track},
  timecode::Timecode,
};
use uuid::Uuid;

pub struct Project {
  pub id: Uuid,
  pub name: Option<String>,
  pub file: File,
  pub dirty: bool,
  pub length: Timecode,
}

impl Default for Project {
  fn default() -> Self {
    let mut file = File::default();
    file.tracks.push(Track::new_lyrics(0));

    Self {
      id: Uuid::new_v4(),
      name: None,
      file,
      dirty: true,
      length: Timecode(0),
    }
  }
}

impl Project {
  pub fn from_file(id: Uuid, name: String, file: File) -> Project {
    Project {
      id,
      name: Some(name),
      length: file.calculate_length(),
      file,
      dirty: false,
    }
  }
}
