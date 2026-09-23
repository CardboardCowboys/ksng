use crate::{
  KsngContext,
  modals::KModal,
  util::r#async::{AsyncResult, AsyncValue},
};
use rfd::{AsyncFileDialog, FileHandle};
use std::path::PathBuf;

type OpenFileCallback = dyn Fn(&KsngContext, PathBuf);

pub struct OpenFileModal {
  open: bool,
  dialog: AsyncValue<Option<FileHandle>>,
  after: Box<OpenFileCallback>,
}

impl OpenFileModal {
  pub fn new<F>(filter_name: String, extensions: Vec<&'static str>, after: F) -> Self
  where
    F: Fn(&KsngContext, PathBuf) + 'static,
  {
    let dialog = AsyncValue::new(Self::pick_file(filter_name, extensions));

    OpenFileModal {
      open: true,
      dialog,
      after: Box::new(after),
    }
  }

  async fn pick_file(filter_name: String, extensions: Vec<&'static str>) -> Option<FileHandle> {
    let mut dialog = AsyncFileDialog::new().add_filter(&filter_name, &extensions);

    if let Some(home_dir) = directories::UserDirs::new().map(|u| u.home_dir().to_path_buf()) {
      dialog = dialog.set_directory(home_dir);
    }

    dialog.pick_file().await
  }
}

impl KModal for OpenFileModal {
  fn should_cleanup(&self) -> bool {
    !self.open
  }

  fn process(&mut self, app: &KsngContext, _context: &egui::Context) {
    if !self.open {
      return;
    }

    match self.dialog.poll() {
      AsyncResult::Pending => {}
      AsyncResult::Complete(Some(file)) => {
        (self.after)(app, file.path().to_path_buf());
      }
      AsyncResult::Complete(None) => {
        self.open = false;
      }
    }
  }
}
