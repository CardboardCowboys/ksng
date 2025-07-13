use std::path::Path;

use infer::Infer;
use klib::{objects::audio::AudioFileType, timecode::Timecode};
use symphonia::core::{
  formats::{FormatOptions, FormatReader},
  io::MediaSourceStream,
  meta::MetadataOptions,
  probe::Hint,
};

use crate::util::error::UiError;

pub struct AudioFileInfo {
  pub audio_type: AudioFileType,
  pub length: Timecode,
  pub sample_rate: u32,
  pub channels: usize,
}

impl AudioFileInfo {
  pub fn from_file(path: &Path) -> Result<Option<AudioFileInfo>, UiError> {
    let file_type = Infer::new()
      .get_from_path(path)?
      .ok_or(UiError::Io("Failed to detect file type".to_string()))?;

    let audio_type = Self::file_type_from_mime(file_type.mime_type());
    if audio_type.is_none() {
      return Ok(None);
    }

    let file = std::fs::File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    // Create a probe hint using the file's extension. [Optional]
    let mut hint = Hint::new();
    hint.mime_type(file_type.mime_type());

    let meta_opts: MetadataOptions = Default::default();
    let fmt_opts: FormatOptions = Default::default();

    let probed = symphonia::default::get_probe().format(&hint, mss, &fmt_opts, &meta_opts)?;

    Ok(Self::info_from_format(
      probed.format.as_ref(),
      audio_type.unwrap(),
    ))
  }

  fn info_from_format(
    format: &dyn FormatReader,
    audio_type: AudioFileType,
  ) -> Option<AudioFileInfo> {
    let codec_params = &format.tracks().first()?.codec_params;
    let time_base = codec_params.time_base?;
    let len = time_base.calc_time(codec_params.n_frames?);
    let len_ms = (len.seconds as u32) * 1000 + (len.frac * 1000.0) as u32;

    Some(AudioFileInfo {
      audio_type,
      length: Timecode(len_ms),
      sample_rate: codec_params.sample_rate?,
      channels: codec_params.channels.iter().len(),
    })
  }

  fn file_type_from_mime(mime: &str) -> Option<AudioFileType> {
    if mime == "audio/mpeg" {
      return Some(AudioFileType::Mp3);
    } else if mime == "audio/x-flac" {
      return Some(AudioFileType::Flac);
    } else if mime == "audio/ogg" {
      return Some(AudioFileType::Ogg);
    } else if mime == "audio/x-wav" {
      return Some(AudioFileType::Wave);
    } else if mime == "audio/aac" {
      return Some(AudioFileType::Aac);
    }

    None
  }
}
