use std::{
  path::Path,
  sync::{Arc, RwLock},
};

use crate::{
  error::Error,
  objects::{attachment::AttachmentResolver, file::File},
  video::export::ffmpeg::FfmpegEncoder,
};

mod ffmpeg;

pub use crate::video::export::ffmpeg::{FfmpegCodecSet, FfmpegEncoderOptions};

#[derive(Default)]
pub enum VideoExportStatus {
  #[default]
  InProgress,
  Completed,
  Cancelled,
  Failed(Error),
}

#[derive(Default)]
pub struct VideoExportProgressMonitor {
  pub num_steps: usize,
  pub current_step: RwLock<usize>,
  pub total: RwLock<usize>,
  pub progress: RwLock<usize>,
  pub step_name: RwLock<String>,
  pub status: RwLock<VideoExportStatus>,
  pub cancelled: RwLock<bool>,
}

impl VideoExportProgressMonitor {
  pub fn new(total_steps: usize, step_name: String, total: usize) -> VideoExportProgressMonitor {
    VideoExportProgressMonitor {
      num_steps: total_steps,
      total: RwLock::new(total),
      step_name: RwLock::new(step_name),
      ..Default::default()
    }
  }

  pub fn next_step(&self, name: String, new_total: usize) {
    *self.total.write().unwrap() = new_total;
    *self.step_name.write().unwrap() = name;
    *self.current_step.write().unwrap() += 1;
  }

  /// Returns the completion percent of this monitor between 0.0 and 1.0.
  pub fn percent(&self) -> f32 {
    let progress = self.progress.read().map(|v| *v).unwrap_or(0);
    let total = self.total.read().map(|v| *v).unwrap_or(0);
    (progress as f32 / total as f32).clamp(0.0, 1.0)
  }
}

pub trait VideoExporter {
  fn export(&self) -> Result<Arc<VideoExportProgressMonitor>, Error>;
}

pub fn create_exporter_ffmpeg(
  options: &FfmpegEncoderOptions,
  file: &File,
  attachment_resolver: &dyn AttachmentResolver,
  output_path: &Path,
) -> Result<Box<dyn VideoExporter>, Error> {
  Ok(Box::new(FfmpegEncoder::new(
    options,
    file,
    attachment_resolver,
    output_path,
  )?))
}
