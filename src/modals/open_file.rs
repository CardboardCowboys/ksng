use crate::{modals::KModal, KsngApp};
use egui_file_dialog::{DialogState, FileDialog};
use std::path::PathBuf;

type OpenFileCallback = dyn Fn(&KsngApp, PathBuf);

pub struct OpenFileModal {
  open: bool,
  dialog: FileDialog,
  after: Box<OpenFileCallback>,
}

impl OpenFileModal {
  pub fn new<F>(filter_name: String, extensions: Vec<&'static str>, after: F) -> Self
  where
    F: Fn(&KsngApp, PathBuf) + 'static,
  {
    let mut dialog = FileDialog::new()
      .add_file_filter_extensions(&filter_name, extensions)
      .as_modal(true);

    if let Some(home_dir) = directories::UserDirs::new().map(|u| u.home_dir().to_path_buf()) {
      dialog = dialog.initial_directory(home_dir);
    }

    dialog.pick_file();

    OpenFileModal {
      open: true,
      dialog,
      after: Box::new(after),
    }
  }
}

impl KModal for OpenFileModal {
  fn should_cleanup(&self) -> bool {
    !self.open
  }

  fn process(&mut self, app: &KsngApp, context: &egui::Context) {
    if !self.open {
      return;
    }

    let res = self.dialog.update(context);
    if let Some(path) = res.picked() {
      (self.after)(app, path.to_path_buf());
    }

    if res.state() != DialogState::Open {
      self.open = false;
    }
  }
}
