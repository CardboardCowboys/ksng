use std::{
  path::{Path, PathBuf},
  sync::{Arc, Mutex},
};

use ffmpeg_next::{
  codec::{self, Context},
  encoder::{self, Encoder},
  rescale::TIME_BASE,
  util::format,
  ChannelLayout, Dictionary, Rational,
};
use zerocopy::IntoBytes;

use crate::{
  audio::mixer_stream::{self, AudioMixerStream},
  error::Error,
  objects::file::File,
  timecode::Timecode,
  video::{
    export::{VideoExportProgressMonitor, VideoExportStatus, VideoExporter},
    renderer::VideoRenderer,
    sequence::VideoSequence,
    VideoConfig,
  },
};

pub struct FfmpegEncoder {
  options: FfmpegEncoderOptions,
  output_path: PathBuf,
  shared: Arc<Mutex<FfmpegEncoderSharedState>>,
  duration: Timecode,
}

#[derive(Debug)]
enum FfmpegCodecSet {
  Mp4H264Aac,
  Mp4Av1Aac,
  WebmVp9Opus,
  WebmAv1Opus,
}

pub struct FfmpegEncoderOptions {
  codec_set: FfmpegCodecSet,
  frame_rate: usize,
  audio_opts: String,
  video_opts: String,
}

struct FfmpegEncoderSharedState {
  video_encoder: encoder::Video,
  audio_encoder: encoder::Audio,
  mixer: AudioMixerStream,
  out_stream: ffmpeg_next::format::context::Output,
  config: VideoConfig,
  sequence: VideoSequence,
}

const SAMPLE_RATE: usize = 44100;

impl FfmpegEncoder {
  pub fn new(
    options: FfmpegEncoderOptions,
    file: &File,
    output_path: &Path,
  ) -> Result<FfmpegEncoder, Error> {
    let output_path = match options.codec_set {
      FfmpegCodecSet::Mp4Av1Aac | FfmpegCodecSet::Mp4H264Aac => output_path.with_extension("mp4"),
      FfmpegCodecSet::WebmAv1Opus | FfmpegCodecSet::WebmVp9Opus => {
        output_path.with_extension("webm")
      }
    };

    let mut out_stream = ffmpeg_next::format::output(&output_path)?;

    let video_codec = match options.codec_set {
      FfmpegCodecSet::Mp4Av1Aac | FfmpegCodecSet::WebmAv1Opus => encoder::find(codec::Id::AV1),
      FfmpegCodecSet::WebmVp9Opus => encoder::find(codec::Id::VP9),
      FfmpegCodecSet::Mp4H264Aac => encoder::find(codec::Id::H264),
    }
    .ok_or(Error::VideoExport(format!(
      "Could not find video codec for {:?}",
      options.codec_set
    )))?;

    let mut video_ost = out_stream.add_stream(video_codec)?;
    let mut video_encoder = codec::context::Context::new_with_codec(video_codec)
      .encoder()
      .video()?;
    video_ost.set_parameters(&video_encoder);

    video_encoder.set_width(file.config.video.width as u32);
    video_encoder.set_height(file.config.video.height as u32);
    video_encoder
      .set_aspect_ratio(file.config.video.width as f64 / file.config.video.height as f64);
    video_encoder.set_format(format::Pixel::RGBAF32LE);
    video_encoder.set_frame_rate(Some(Rational::new(1, options.frame_rate as i32)));
    video_encoder.set_time_base(TIME_BASE);

    let audio_codec = match options.codec_set {
      FfmpegCodecSet::Mp4Av1Aac | FfmpegCodecSet::Mp4H264Aac => encoder::find(codec::Id::AAC),
      FfmpegCodecSet::WebmAv1Opus | FfmpegCodecSet::WebmVp9Opus => encoder::find(codec::Id::OPUS),
    }
    .ok_or(Error::VideoExport(format!(
      "Could not find audio codec for {:?}",
      options.codec_set
    )))?;

    let mut audio_ost = out_stream.add_stream(audio_codec)?;
    let mut audio_encoder = codec::context::Context::new_with_codec(audio_codec)
      .encoder()
      .audio()?;
    audio_ost.set_parameters(&audio_encoder);
    audio_encoder.set_bit_rate(128000);
    audio_encoder.set_channel_layout(ChannelLayout::STEREO);
    audio_encoder.set_time_base(Rational::new(1, SAMPLE_RATE as i32));

    let audio_opts = Self::parse_opts(&options.audio_opts).ok_or(Error::VideoExport(
      "Could not parse audio options string".to_owned(),
    ))?;
    let video_opts = Self::parse_opts(&options.video_opts).ok_or(Error::VideoExport(
      "Could not parse video options string".to_owned(),
    ))?;

    let audio_encoder = audio_encoder.open_with(audio_opts)?;
    let video_encoder = video_encoder.open_with(video_opts)?;

    let mut mixer = AudioMixerStream::new(2, 44100)?;
    mixer.update_from_tracks(&file.tracks)?;

    let sequence = VideoSequence::from_file(file, &file.config.video);

    Ok(FfmpegEncoder {
      options,
      output_path,
      duration: file.calculate_length(),
      shared: Arc::new(Mutex::new(FfmpegEncoderSharedState {
        video_encoder,
        audio_encoder,
        mixer,
        out_stream,
        config: file.config.video.clone(),
        sequence,
      })),
    })
  }

  fn parse_opts<'a>(s: &str) -> Option<Dictionary<'a>> {
    let mut dict = Dictionary::new();
    for keyval in s.split_terminator(',') {
      let tokens: Vec<&str> = keyval.split('=').collect();
      match tokens[..] {
        [key, val] => dict.set(key, val),
        _ => return None,
      }
    }
    Some(dict)
  }

  fn encode_audio_impl(
    total: usize,
    monitor: Arc<VideoExportProgressMonitor>,
    shared: Arc<Mutex<FfmpegEncoderSharedState>>,
  ) -> Result<(), Error> {
    let mut shared = shared.lock().unwrap();
    let mut finished_samples = 0;

    let buffer_size = mixer_stream::BLOCK_SIZE * 2;
    let mut buffer = Vec::with_capacity(buffer_size);
    buffer.resize(buffer_size, 0.0_f32);

    let mut frame = ffmpeg_next::frame::Audio::new(
      format::Sample::F32(format::sample::Type::Packed),
      buffer_size,
      ChannelLayout::STEREO,
    );

    while finished_samples < total {
      if *monitor.cancelled.read().unwrap() {
        *monitor.status.write().unwrap() = VideoExportStatus::Cancelled;
        return Ok(());
      }

      let num_samples = shared.mixer.process(&mut buffer)?;

      let frame_data = &mut frame.data_mut(0)[0..num_samples];
      frame_data.copy_from_slice(buffer[0..num_samples].as_bytes());
      frame.set_samples(num_samples / 2);

      shared.audio_encoder.send_frame(&frame)?;
      let mut packet = ffmpeg_next::Packet::empty();
      while shared.audio_encoder.receive_packet(&mut packet).is_ok() {
        packet.set_stream(1);
        packet.write_interleaved(&mut shared.out_stream)?;
      }

      finished_samples += num_samples;
      *monitor.progress.write().unwrap() = finished_samples;
    }

    shared.audio_encoder.send_eof()?;

    *monitor.status.write().unwrap() = VideoExportStatus::Completed;
    Ok(())
  }

  fn encode_video_impl(
    frame_rate: usize,
    total: usize,
    monitor: Arc<VideoExportProgressMonitor>,
    shared: Arc<Mutex<FfmpegEncoderSharedState>>,
  ) -> Result<(), Error> {
    let mut shared = shared.lock().unwrap();
    let mut finished_frames = 0;

    let mut buffer = Vec::new();
    VideoRenderer::allocate_buffer(&shared.config, &mut buffer);

    let mut frame = ffmpeg_next::frame::Video::new(
      format::Pixel::RGBAF32LE,
      shared.config.width as u32,
      shared.config.height as u32,
    );

    let mut renderer = VideoRenderer::new()?;

    while finished_frames < total {
      if *monitor.cancelled.read().unwrap() {
        *monitor.status.write().unwrap() = VideoExportStatus::Cancelled;
        return Ok(());
      }

      let time = Timecode::from_seconds_f64(finished_frames as f64 / frame_rate as f64);

      renderer.render_frame(&shared.config, &shared.sequence, time, &mut buffer)?;

      frame.data_mut(0).copy_from_slice(&buffer);
      frame.set_pts(Some(finished_frames as i64));
      frame.set_kind(ffmpeg_next::picture::Type::None);

      shared.video_encoder.send_frame(&frame)?;
      let mut packet = ffmpeg_next::Packet::empty();
      while shared.video_encoder.receive_packet(&mut packet).is_ok() {
        packet.set_stream(0);
        packet.write_interleaved(&mut shared.out_stream)?;
      }

      finished_frames += 1;
      *monitor.progress.write().unwrap() = finished_frames;
    }

    shared.video_encoder.send_eof()?;

    *monitor.status.write().unwrap() = VideoExportStatus::Completed;
    Ok(())
  }
}

impl VideoExporter for FfmpegEncoder {
  fn encode_audio(&self) -> Result<Arc<VideoExportProgressMonitor>, Error> {
    let total = (self.duration.to_seconds_f64() * SAMPLE_RATE as f64).ceil() as usize;
    let monitor = Arc::new(VideoExportProgressMonitor::new(total));
    let monitor_ret = monitor.clone();
    let shared = self.shared.clone();

    std::thread::spawn(move || {
      let monitor_copy = monitor.clone();
      if let Err(error) = Self::encode_audio_impl(total, monitor, shared) {
        *monitor_copy.status.write().unwrap() = VideoExportStatus::Failed(error);
      }
    });

    Ok(monitor_ret)
  }

  fn encode_video(&self) -> Result<Arc<VideoExportProgressMonitor>, Error> {
    let frame_rate = self.options.frame_rate;
    let total = (self.duration.to_seconds_f64() * frame_rate as f64).ceil() as usize;
    let monitor = Arc::new(VideoExportProgressMonitor::new(total));
    let monitor_ret = monitor.clone();
    let shared = self.shared.clone();

    std::thread::spawn(move || {
      let monitor_copy = monitor.clone();
      if let Err(error) = Self::encode_video_impl(frame_rate, total, monitor, shared) {
        *monitor_copy.status.write().unwrap() = VideoExportStatus::Failed(error);
      }
    });

    Ok(monitor_ret)
  }
}
