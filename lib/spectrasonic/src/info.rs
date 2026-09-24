use std::path::Path;

use crate::{Error, Timecode};

pub struct AudioFileInfo {
  pub mime_type: String,
  pub sample_rate: usize,
  pub channels: usize,
  pub duration: Timecode,
}

#[cfg(feature = "ffmpeg")]
fn get_info(path: &Path, mime_type: String) -> Result<Option<AudioFileInfo>, Error> {
  let input = ffmpeg_next::format::input(path)?;

  let Some(in_stream) = input.streams().best(ffmpeg_next::media::Type::Audio) else {
    return Ok(None);
  };

  let context = ffmpeg_next::codec::context::Context::from_parameters(in_stream.parameters())?;
  let decoder = context.decoder().audio()?;
  let channels = decoder.channels() as usize;
  let sample_rate = decoder.rate() as usize;
  let time_base = decoder.time_base();
  let duration = in_stream.duration();

  Ok(Some(AudioFileInfo {
    mime_type,
    sample_rate,
    channels,
    duration: Timecode {
      sample_rate: time_base.1 as usize,
      samples: duration as usize * time_base.0 as usize,
    },
  }))
}

#[cfg(all(feature = "symphonia", not(feature = "ffmpeg")))]
fn get_info(path: &Path, mime_type: String) -> Result<Option<AudioFileInfo>, Error> {
  use symphonia::core::{
    formats::FormatOptions, io::MediaSourceStream, meta::MetadataOptions, probe::Hint,
  };

  let file = std::fs::File::open(path)?;
  let mss = MediaSourceStream::new(Box::new(file), Default::default());

  let mut hint = Hint::new();
  hint.mime_type(&mime_type);

  let meta_opts: MetadataOptions = Default::default();
  let fmt_opts: FormatOptions = Default::default();

  let probed = symphonia::default::get_probe().format(&hint, mss, &fmt_opts, &meta_opts)?;

  let Some(track) = &probed.format.tracks().first() else {
    return Ok(None);
  };

  let Some(n_frames) = track.codec_params.n_frames else {
    return Ok(None);
  };
  let Some(sample_rate) = track.codec_params.sample_rate else {
    return Ok(None);
  };
  let Some(channels) = track.codec_params.channels else {
    return Ok(None);
  };

  let delay = track.codec_params.delay.unwrap_or(0);
  let padding = track.codec_params.padding.unwrap_or(0);

  Ok(Some(AudioFileInfo {
    mime_type,
    sample_rate: sample_rate as usize,
    channels: channels.count(),
    duration: Timecode {
      samples: n_frames as usize - delay as usize - padding as usize,
      sample_rate: sample_rate as usize,
    },
  }))
}

#[cfg(all(not(feature = "ffmpeg"), not(feature = "symphonia")))]
fn get_info(path: &Path, mime_type: String) -> Result<Option<AudioFileInfo>, Error> {
  log::warn!("spectrasonic built without any decoder features, returning None for get_info");
  Ok(None)
}

pub fn get_file_info<P: AsRef<Path>>(input: P) -> Result<Option<AudioFileInfo>, Error> {
  let mime_type = infer::Infer::new()
    .get_from_path(input.as_ref())?
    .map(|t| t.mime_type().to_string());

  let Some(mime_type) = mime_type else {
    return Ok(None);
  };

  get_info(input.as_ref(), mime_type)
}
