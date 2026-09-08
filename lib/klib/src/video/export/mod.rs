use std::{
  path::Path,
  sync::{Arc, RwLock},
};

use crate::{
  error::Error,
  objects::file::File,
  video::export::ffmpeg::{FfmpegEncoder, FfmpegEncoderOptions},
};

mod ffmpeg;

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
}

pub trait VideoExporter {
  fn encode_audio(&self) -> Result<Arc<VideoExportProgressMonitor>, Error>;
  fn encode_video(&self) -> Result<Arc<VideoExportProgressMonitor>, Error>;
}

pub fn create_exporter_ffmpeg(
  options: FfmpegEncoderOptions,
  file: &File,
  output_path: &Path,
) -> Result<Box<dyn VideoExporter>, Error> {
  Ok(Box::new(FfmpegEncoder::new(options, file, output_path)?))
}
