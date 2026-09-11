use std::{path::PathBuf, str::FromStr, sync::Arc};

use crate::{components::config_editor, modals::KModal};
use egui::{Atom, Button, Color32, Id, Modal, ProgressBar, Spinner, Vec2};
use egui_file_dialog::{DialogState, FileDialog};
use klib::video::export::{self, FfmpegEncoderOptions, VideoExportProgressMonitor, VideoExporter};

enum VideoExportSetting {
  Ffmpeg(FfmpegEncoderOptions),
}

enum ModalState {
  Opened,
  Exporting(Arc<VideoExportProgressMonitor>),
  Failed(klib::error::Error),
  Complete,
  Cancelled,
}

pub struct ExportVideoModal {
  open: bool,
  setting: VideoExportSetting,
  output_path: Option<PathBuf>,
  dialog: FileDialog,
  exporter: Option<Box<dyn VideoExporter>>,
  state: ModalState,
}

impl ExportVideoModal {
  pub fn new(project_name: String) -> Self {
    let mut dialog = FileDialog::new().as_modal(false);

    if let Some(home_dir) = directories::UserDirs::new().map(|u| u.home_dir().to_path_buf()) {
      dialog = dialog.initial_directory(home_dir);
    }

    let mut output_path = None;
    if let Some(desktop_dir) =
      directories::UserDirs::new().and_then(|u| u.desktop_dir().map(|p| p.to_path_buf()))
    {
      output_path = Some(desktop_dir.join(project_name));
    }

    ExportVideoModal {
      open: true,
      dialog,
      output_path,
      setting: VideoExportSetting::Ffmpeg(Default::default()),
      exporter: None,
      state: ModalState::Opened,
    }
  }
}

impl KModal for ExportVideoModal {
  fn should_cleanup(&self) -> bool {
    !self.open
  }

  fn process(&mut self, app: &crate::KsngApp, context: &egui::Context) {
    if !self.open {
      return;
    }

    let project = app.project.borrow();
    let Some(project) = project.as_ref() else {
      self.open = false;
      return;
    };

    let modal = Modal::new(Id::new("modal#confirm")).show(context, |ui| {
      ui.set_width(250.0);

      ui.heading("Export Video");
      ui.separator();

      // Show options if we are not currently exporting a video.

      let is_settings_enabled = matches!(
        self.state,
        ModalState::Opened | ModalState::Cancelled | ModalState::Complete | ModalState::Failed(..)
      );

      let mut output_path = self
        .output_path
        .as_ref()
        .and_then(|p| p.to_str())
        .map(|p| p.to_owned())
        .unwrap_or_default();

      ui.add_enabled_ui(is_settings_enabled, |ui| {
        ui.horizontal(|ui| {
          ui.label("Output File");
          ui.text_edit_singleline(&mut output_path);
          if ui.button("..").clicked() {
            self.dialog.save_file();
          }
        });

        if let Some(picked) = self.dialog.take_picked() {
          self.output_path = Some(picked.to_path_buf());
        } else if !output_path.is_empty()
          && let Ok(output_path) = PathBuf::from_str(&output_path)
        {
          self.output_path = Some(output_path);
        }

        let mut selected_encoder = match self.setting {
          VideoExportSetting::Ffmpeg(..) => "FFmpeg",
        }
        .to_owned();

        let prev_selected_encoder = selected_encoder.clone();

        egui::ComboBox::from_label("Encoder")
          .selected_text(selected_encoder.clone())
          .show_ui(ui, |ui| {
            ui.selectable_value(&mut selected_encoder, "FFmpeg".to_owned(), "FFmpeg")
          });

        if selected_encoder != prev_selected_encoder && selected_encoder == "FFmpeg" {
          self.setting = VideoExportSetting::Ffmpeg(Default::default())
        }

        match &mut self.setting {
          VideoExportSetting::Ffmpeg(ffmpeg_encoder_options) => config_editor::config_editor(
            ui,
            "export#encoder_settings".to_owned(),
            ffmpeg_encoder_options,
          ),
        };
      });

      let mut new_state = None;

      match &self.state {
        ModalState::Opened => {}
        ModalState::Exporting(monitor) => {
          ui.label(format!(
            "{} ({}/{})",
            monitor.step_name.read().unwrap(),
            *monitor.current_step.read().unwrap() + 1,
            monitor.num_steps
          ));
          ui.add(
            ProgressBar::new(monitor.percent())
              .animate(false)
              .show_percentage(),
          );

          match &*monitor.status.read().unwrap() {
            export::VideoExportStatus::InProgress => {}
            export::VideoExportStatus::Completed => {
              new_state = Some(ModalState::Complete);
            }
            export::VideoExportStatus::Cancelled => {
              new_state = Some(ModalState::Cancelled);
            }
            export::VideoExportStatus::Failed(error) => {
              new_state = Some(ModalState::Failed(error.clone()));
            }
          }
        }
        ModalState::Failed(error) => {
          ui.label("Error while exporting");
          ui.colored_label(Color32::RED, error.to_string());
        }
        ModalState::Complete => {
          ui.label("Completed");
        }
        ModalState::Cancelled => {
          ui.label("Cancelled");
        }
      };

      if let Some(new_state) = new_state {
        self.state = new_state;
      }

      ui.horizontal(|ui| {
        if ui.button("Cancel").clicked() {
          match &self.state {
            ModalState::Exporting(monitor) => {
              *monitor.cancelled.write().unwrap() = true;
              self.state = ModalState::Cancelled;
            }
            _ => {
              self.open = false;
            }
          }
        }

        if is_settings_enabled {
          let has_output_path = self.output_path.is_some();
          if ui
            .add_enabled(has_output_path, Button::new("Start"))
            .clicked()
          {
            let exporter = match &self.setting {
              VideoExportSetting::Ffmpeg(ffmpeg_encoder_options) => export::create_exporter_ffmpeg(
                ffmpeg_encoder_options,
                &project.file,
                self.output_path.as_ref().unwrap(),
              ),
            };

            match exporter {
              Err(err) => self.state = ModalState::Failed(err),
              Ok(exporter) => {
                match exporter.export() {
                  Ok(monitor) => {
                    self.state = ModalState::Exporting(monitor);
                  }
                  Err(err) => {
                    self.state = ModalState::Failed(err);
                  }
                }

                self.exporter = Some(exporter);
              }
            }
          }
        } else {
          let id = Id::new("export#waiting");
          let response = Button::new(Atom::custom(id, Vec2::splat(18.0))).atom_ui(ui);
          let rect = response.rect(id);
          if let Some(rect) = rect {
            ui.place(rect, Spinner::new());
          }
        }
      });
    });

    if *self.dialog.state() == DialogState::Open {
      self.dialog.update(context);
    }
  }
}
