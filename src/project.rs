use klib::objects::{file::File, track::Track};
use uuid::Uuid;

pub struct Project {
  pub id: Uuid,
  pub name: Option<String>,
  pub file: File,
  pub dirty: bool,
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
    }
  }
}
