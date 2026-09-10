use std::{
  path::Path,
  sync::{Arc, RwLock},
};

use crate::{error::Error, objects::file::File, video::export::ffmpeg::FfmpegEncoder};

mod ffmpeg;

pub use crate::video::export::ffmpeg::FfmpegEncoderOptions;

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
  pub total: usize,
  pub progress: RwLock<usize>,
  pub status: RwLock<VideoExportStatus>,
  pub cancelled: RwLock<bool>,
}

impl VideoExportProgressMonitor {
  pub fn new(total: usize) -> VideoExportProgressMonitor {
    VideoExportProgressMonitor {
      total,
      ..Default::default()
    }
  }

  /// Returns the completion percent of this monitor between 0.0 and 1.0.
  pub fn percent(&self) -> f32 {
    let progress = self.progress.read().map(|v| *v).unwrap_or(0);
    (progress as f32 / self.total as f32).clamp(0.0, 1.0)
  }
}

pub trait VideoExporter {
  fn encode_audio(&self) -> Result<Arc<VideoExportProgressMonitor>, Error>;
  fn encode_video(&self) -> Result<Arc<VideoExportProgressMonitor>, Error>;
}

pub fn create_exporter_ffmpeg(
  options: &FfmpegEncoderOptions,
  file: &File,
  output_path: &Path,
) -> Result<Box<dyn VideoExporter>, Error> {
  Ok(Box::new(FfmpegEncoder::new(options, file, output_path)?))
}
