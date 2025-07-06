use klib::objects::file::File;
use uuid::Uuid;

#[derive(Clone)]
pub struct Project {
  pub id: Uuid,
  pub name: Option<String>,
  pub file: File,
  pub dirty: bool,
}

impl Default for Project {
  fn default() -> Self {
    Self {
      id: Uuid::new_v4(),
      name: None,
      file: Default::default(),
      dirty: true,
    }
  }
}
