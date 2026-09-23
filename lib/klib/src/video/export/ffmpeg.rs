use std::{
  path::{Path, PathBuf},
  sync::{Arc, Mutex, Once, RwLock},
};

use ffmpeg_next::{
  codec::{self},
  encoder::{self},
  util::format::{self, sample::Sample},
  ChannelLayout, Dictionary, Rational,
};
use klib_macros::EditableConfig;
use zerocopy::IntoBytes;

use crate::{
  audio::mixer_stream::AudioMixerStream,
  error::Error,
  objects::{attachment::AttachmentResolver, file::File},
  timecode::Timecode,
  video::{
    export::{VideoExportProgressMonitor, VideoExportStatus, VideoExporter},
    renderer::VideoRenderer,
    sequence::VideoSequence,
    VideoConfig,
  },
};

pub struct FfmpegEncoder {
  shared: Arc<Mutex<FfmpegEncoderSharedState>>,
}

#[derive(EditableConfig, Debug, Copy, Clone)]
pub enum FfmpegCodecSet {
  Mp4H264Aac,
  Mp4Av1Aac,
  WebmVp9Opus,
  WebmAv1Opus,
}

impl FfmpegCodecSet {
  fn sample_format(&self) -> Sample {
    match &self {
      Self::Mp4H264Aac | Self::Mp4Av1Aac => format::Sample::F32(format::sample::Type::Planar),
      Self::WebmVp9Opus | Self::WebmAv1Opus => format::Sample::F32(format::sample::Type::Packed),
    }
  }

  pub fn extension(&self) -> &'static str {
    match &self {
      Self::Mp4Av1Aac | Self::Mp4H264Aac => "mp4",
      Self::WebmAv1Opus | Self::WebmVp9Opus => "webm",
    }
  }
}

#[derive(EditableConfig, Clone)]
pub struct FfmpegEncoderOptions {
  pub codec_set: FfmpegCodecSet,
  pub frame_rate: usize,
  pub audio_opts: String,
  pub video_opts: String,
}

impl Default for FfmpegEncoderOptions {
  fn default() -> Self {
    Self {
      codec_set: FfmpegCodecSet::Mp4H264Aac,
      frame_rate: 30,
      audio_opts: Default::default(),
      video_opts: Default::default(),
    }
  }
}

struct FfmpegEncoderSharedState {
  mixer: RwLock<AudioMixerStream>,
  config: VideoConfig,
  sequence: VideoSequence,
  options: FfmpegEncoderOptions,
  output_path: PathBuf,
  duration: Timecode,
  sample_rate: usize,
  block_size: usize,
}

static FFMPEG_INIT: Once = Once::new();

impl FfmpegEncoder {
  pub fn new(
    options: &FfmpegEncoderOptions,
    file: &File,
    attachment_resolver: &dyn AttachmentResolver,
    output_path: &Path,
  ) -> Result<FfmpegEncoder, Error> {
    FFMPEG_INIT.call_once(|| {
      ffmpeg_next::init().unwrap();
    });

    let (sample_rate, block_size) = match &options.codec_set {
      FfmpegCodecSet::Mp4Av1Aac | FfmpegCodecSet::Mp4H264Aac => (44100_usize, 1024),
      FfmpegCodecSet::WebmVp9Opus | FfmpegCodecSet::WebmAv1Opus => (48000_usize, 960),
    };

    let mut mixer = AudioMixerStream::new(2, sample_rate, block_size)?;
    mixer.update_from_tracks(file, &file.tracks, attachment_resolver)?;

    let sequence = VideoSequence::from_file(file, &file.config.video);

    Ok(FfmpegEncoder {
      shared: Arc::new(Mutex::new(FfmpegEncoderSharedState {
        options: options.clone(),
        output_path: output_path.to_path_buf(),
        duration: file.calculate_length(),
        mixer: RwLock::new(mixer),
        config: file.config.video.clone(),
        sequence,
        sample_rate,
        block_size,
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

  fn export_impl(
    monitor: Arc<VideoExportProgressMonitor>,
    shared: Arc<Mutex<FfmpegEncoderSharedState>>,
  ) -> Result<(), Error> {
    let shared = shared.lock().unwrap();
    let options = &shared.options;
    let config = &shared.config;
    let sample_rate = shared.sample_rate;

    // Step 1: Initializing
    let output_path = shared
      .output_path
      .with_extension(options.codec_set.extension());

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

    video_encoder.set_width(config.width as u32);
    video_encoder.set_height(config.height as u32);
    video_encoder.set_format(format::Pixel::YUV420P);
    //video_encoder.set_frame_rate(Some(Rational::new(options.frame_rate as
    // i32, 1)));
    video_encoder.set_time_base(Rational::new(1, options.frame_rate as i32));
    video_encoder.set_max_b_frames(1);
    video_encoder.set_gop(10);
    video_ost.set_time_base(Rational::new(1, options.frame_rate as i32));
    video_ost.set_parameters(&video_encoder);

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
    audio_encoder.set_bit_rate(256000);
    audio_encoder.set_rate(sample_rate as i32);
    audio_encoder.set_format(options.codec_set.sample_format());
    audio_encoder.set_channel_layout(ChannelLayout::STEREO);
    audio_encoder.set_time_base(Rational::new(1, sample_rate as i32));
    audio_ost.set_parameters(&audio_encoder);

    let audio_opts = Self::parse_opts(&options.audio_opts).ok_or(Error::VideoExport(
      "Could not parse audio options string".to_owned(),
    ))?;
    let video_opts = Self::parse_opts(&options.video_opts).ok_or(Error::VideoExport(
      "Could not parse video options string".to_owned(),
    ))?;

    let mut audio_encoder = audio_encoder.open_with(audio_opts)?;
    let mut video_encoder = video_encoder.open_with(video_opts)?;

    out_stream.write_header()?;

    *monitor.total.write().unwrap() = 1;
    if *monitor.cancelled.read().unwrap() {
      *monitor.status.write().unwrap() = VideoExportStatus::Cancelled;
      return Ok(());
    }

    // Step 2: export audio
    let total = (sample_rate as f64 * shared.duration.to_seconds_f64()).ceil() as usize * 2;
    monitor.next_step("Encoding audio".to_owned(), total);
    {
      let mut finished_samples = 0;
      let block_size = shared.block_size;

      let buffer_size = block_size * 2;
      let mut buffer = Vec::with_capacity(buffer_size);
      buffer.resize(buffer_size, 0.0_f32);

      let is_interleaved = matches!(
        audio_encoder.format(),
        format::sample::Sample::F32(format::sample::Type::Packed)
      );

      let mut frame =
        ffmpeg_next::frame::Audio::new(audio_encoder.format(), block_size, ChannelLayout::STEREO);
      frame.set_rate(sample_rate as u32);

      let mut mixer = shared.mixer.write().unwrap();

      while finished_samples < total {
        if *monitor.cancelled.read().unwrap() {
          *monitor.status.write().unwrap() = VideoExportStatus::Cancelled;
          return Ok(());
        }

        let num_samples = if is_interleaved {
          mixer.process_interleaved(&mut buffer)?
        } else {
          mixer.process_planar(&mut buffer)?
        };

        let num_frames = num_samples / 2;
        let frame_data = &mut frame.data_mut(0)[0..num_samples * size_of::<f32>()];
        frame_data.copy_from_slice(buffer[0..num_samples].as_bytes());
        frame.set_samples(num_frames);
        frame.set_pts(Some((finished_samples as i64) / 2));

        audio_encoder.send_frame(&frame)?;

        let mut packet = ffmpeg_next::Packet::empty();
        while audio_encoder.receive_packet(&mut packet).is_ok() {
          packet.set_stream(1);
          packet.write_interleaved(&mut out_stream)?;
        }

        finished_samples += num_samples;
        *monitor.progress.write().unwrap() = finished_samples;
      }
    }

    // Step 3: encoding video
    let total = (options.frame_rate as f64 * shared.duration.to_seconds_f64()).ceil() as usize;
    monitor.next_step("Encoding video".to_owned(), total);
    {
      let mut finished_frames = 0;

      let mut frame = ffmpeg_next::frame::Video::new(
        format::Pixel::RGBA,
        shared.config.width as u32,
        shared.config.height as u32,
      );

      let mut yuv_frame = ffmpeg_next::frame::Video::new(
        format::Pixel::YUV420P,
        shared.config.width as u32,
        shared.config.height as u32,
      );

      let mut renderer = VideoRenderer::new()?;

      let mut pixel_converter = ffmpeg_next::software::converter(
        (shared.config.width as u32, shared.config.height as u32),
        format::Pixel::RGBA,
        format::Pixel::YUV420P,
      )?;

      let time_base = out_stream
        .stream(0)
        .ok_or(Error::VideoExport("Cuold not get video stream".to_owned()))?
        .time_base();
      let tick_len = time_base.0 as f64 / time_base.1 as f64;

      while finished_frames < total {
        if *monitor.cancelled.read().unwrap() {
          *monitor.status.write().unwrap() = VideoExportStatus::Cancelled;
          return Ok(());
        }

        let time = Timecode::from_seconds_f64(finished_frames as f64 / options.frame_rate as f64);

        renderer.render_frame(&shared.config, &shared.sequence, time, frame.data_mut(0))?;

        pixel_converter.run(&frame, &mut yuv_frame)?;

        yuv_frame.set_pts(Some((time.to_seconds_f64() / tick_len) as i64));
        yuv_frame.set_kind(ffmpeg_next::picture::Type::None);

        video_encoder.send_frame(&yuv_frame)?;
        let mut packet = ffmpeg_next::Packet::empty();
        while video_encoder.receive_packet(&mut packet).is_ok() {
          packet.set_stream(0);
          packet.write_interleaved(&mut out_stream)?;
        }

        finished_frames += 1;
        *monitor.progress.write().unwrap() = finished_frames;
      }
    }

    // Step 4: finalizing
    monitor.next_step("Finalizing".to_owned(), 1);

    video_encoder.send_eof()?;
    let mut packet = ffmpeg_next::Packet::empty();
    while video_encoder.receive_packet(&mut packet).is_ok() {
      packet.set_stream(0);
      packet.write_interleaved(&mut out_stream)?;
    }

    audio_encoder.send_eof()?;
    let mut packet = ffmpeg_next::Packet::empty();
    while audio_encoder.receive_packet(&mut packet).is_ok() {
      packet.set_stream(1);
      packet.write_interleaved(&mut out_stream)?;
    }

    out_stream.write_trailer()?;

    Ok(())
  }
}

impl VideoExporter for FfmpegEncoder {
  fn export(&self) -> Result<Arc<VideoExportProgressMonitor>, Error> {
    let monitor = Arc::new(VideoExportProgressMonitor::new(
      4,
      "Initializing".to_owned(),
      1,
    ));
    let monitor_ret = monitor.clone();
    let shared = self.shared.clone();

    std::thread::spawn(move || {
      let monitor_clone = monitor.clone();
      if let Err(err) = FfmpegEncoder::export_impl(monitor, shared) {
        *monitor_clone.status.write().unwrap() = VideoExportStatus::Failed(err);
      } else {
        *monitor_clone.status.write().unwrap() = VideoExportStatus::Completed;
      }
    });

    Ok(monitor_ret)
  }
}
