use std::{io::ErrorKind, path::Path};

use infer::Infer;
use symphonia::core::{
  audio::{SampleBuffer, SignalSpec},
  codecs::{Decoder, DecoderOptions},
  formats::{FormatOptions, FormatReader},
  io::MediaSourceStream,
  meta::MetadataOptions,
  probe::Hint,
  units::Time,
};

use crate::{
  AudioSource, Error, Timecode,
  buffer::PlanarAudioBuffer,
  chain::{AudioChainBuilder, AudioInfo},
};

fn timecode_to_time(seconds: f64) -> Time {
  Time {
    seconds: seconds as u64,
    frac: seconds.fract(),
  }
}

struct SampleBufferWrapper(usize, SampleBuffer<f32>);

impl PlanarAudioBuffer for SampleBufferWrapper {
  fn num_frames(&self) -> usize {
    self.1.len() / self.0
  }

  fn num_channels(&self) -> usize {
    self.0
  }

  fn channel(&self, i: usize) -> &[f32] {
    let num_frames = self.num_frames();
    &self.1.samples()[(i * num_frames)..((i + 1) * num_frames)]
  }

  fn channel_mut(&mut self, i: usize) -> &mut [f32] {
    let num_frames = self.num_frames();
    &mut self.1.samples_mut()[(i * num_frames)..((i + 1) * num_frames)]
  }

  fn fill(&mut self, v: f32) {
    for ch in 0..self.num_channels() {
      self.channel_mut(ch).fill(v);
    }
  }
}

pub struct SymphoniaAudioSource {
  decoder: Box<dyn Decoder>,
  format: Box<dyn FormatReader>,
  track_id: u32,
  duration: Timecode,
  required_offset: usize,
  wrote_frames: usize,
  buffer: SampleBufferWrapper,
  frames_remaining: usize,
  sample_rate: usize,
  num_channels: usize,
  frame_pos: usize,
}

impl SymphoniaAudioSource {
  pub fn new<P: AsRef<Path>>(input: P) -> Result<SymphoniaAudioSource, Error> {
    let file_type = Infer::new()
      .get_from_path(input.as_ref())?
      .ok_or(Error::msg("Failed to detect file type"))?;

    let codecs = symphonia::default::get_codecs();
    let mss = MediaSourceStream::new(Box::new(std::fs::File::open(input)?), Default::default());

    let mut hint = Hint::new();
    hint.mime_type(file_type.mime_type());

    let fmt_opts: FormatOptions = Default::default();
    let meta_opts: MetadataOptions = Default::default();
    let decoder_opts: DecoderOptions = Default::default();

    let probe = symphonia::default::get_probe().format(&hint, mss, &fmt_opts, &meta_opts)?;
    let format = probe.format;
    let track = format.default_track().unwrap();
    let n_frames = track
      .codec_params
      .n_frames
      .ok_or(Error::msg("Couldn't get audio file n_frames"))?;
    let sample_rate = track
      .codec_params
      .sample_rate
      .ok_or(Error::msg("Could not get audio file sample_rate"))?;
    let channels = track
      .codec_params
      .channels
      .ok_or(Error::msg("Could not get audio file channels"))?;
    let decoder = codecs.make(&track.codec_params, &decoder_opts)?;

    let buffer_size = if let Some(max_size) = track.codec_params.max_frames_per_packet {
      max_size * 2
    } else {
      sample_rate as u64 * 2
    };
    let buffer = SampleBuffer::new(buffer_size, SignalSpec::new(sample_rate, channels));

    let track_id = track.id;
    let required_offset = track.codec_params.delay.unwrap_or(0) as usize;
    let padding = track.codec_params.padding.unwrap_or(0) as usize;

    let duration = Timecode {
      samples: (n_frames as usize)
        .saturating_sub(required_offset)
        .saturating_sub(padding),
      sample_rate: sample_rate as usize,
    };

    Ok(SymphoniaAudioSource {
      decoder,
      format,
      track_id,
      duration,
      required_offset,
      wrote_frames: 0,
      buffer: SampleBufferWrapper(channels.count(), buffer),
      frames_remaining: 0,
      sample_rate: sample_rate as usize,
      num_channels: channels.count(),
      frame_pos: 0,
    })
  }
}

impl AudioSource for SymphoniaAudioSource {
  fn read(&mut self, buffer: &mut dyn crate::buffer::PlanarAudioBuffer) -> Result<usize, Error> {
    let mut frames_written = 0;

    if self.frames_remaining > 0 {
      let num = self.frames_remaining.min(buffer.num_frames());
      let from_offset = self.wrote_frames - self.frames_remaining;
      buffer.copy_from_buffer(&self.buffer, from_offset, 0, num);
      self.frames_remaining -= num;
      frames_written += num;
      self.frame_pos += num;
    }

    loop {
      if frames_written >= buffer.num_frames() {
        // Filled buffer entirely from cached data.
        return Ok(frames_written);
      }

      let packet = self.format.next_packet();
      let packet = match packet {
        Ok(p) => p,
        Err(symphonia::core::errors::Error::IoError(e)) => {
          if e.kind() == ErrorKind::UnexpectedEof {
            return Ok(frames_written);
          }

          return Err(symphonia::core::errors::Error::IoError(e).into());
        }
        Err(e) => {
          return Err(e.into());
        }
      };

      if packet.track_id() != self.track_id {
        continue;
      }

      let audio_buf = self.decoder.decode(&packet)?;
      let frames_read = audio_buf.frames();
      self.buffer.1.copy_planar_ref(audio_buf);

      let frames_read = (self.duration.samples - self.frame_pos).min(frames_read);

      if self.required_offset >= frames_read {
        self.required_offset -= frames_read;
      } else if self.required_offset > 0 {
        let num = (frames_read - self.required_offset).min(buffer.num_frames() - frames_written);
        buffer.copy_from_buffer(&self.buffer, self.required_offset, frames_written, num);
        self.required_offset = 0;
        frames_written += num;
        self.frames_remaining = frames_read - num;
        self.frame_pos += num;
        self.wrote_frames = frames_read;
      } else {
        let num = frames_read.min(buffer.num_frames() - frames_written);
        buffer.copy_from_buffer(&self.buffer, 0, frames_written, num);
        frames_written += num;
        self.frames_remaining = frames_read - num;
        self.frame_pos += num;
        self.wrote_frames = frames_read;
      }
    }
  }

  fn seek(&mut self, pos: crate::Timecode) -> Result<(), Error> {
    let mut pos_seconds = pos.to_seconds_f64();
    if pos_seconds == self.duration.to_seconds_f64() {
      pos_seconds = self.duration.to_seconds_f64() - 0.0001;
    }
    let seeked_to = self.format.seek(
      symphonia::core::formats::SeekMode::Accurate,
      symphonia::core::formats::SeekTo::Time {
        time: timecode_to_time(pos_seconds),
        track_id: Some(self.track_id),
      },
    )?;
    self.required_offset = (seeked_to.required_ts - seeked_to.actual_ts) as usize;
    self.frame_pos = seeked_to.actual_ts as usize;
    // Force re-read instead of using cached data
    self.frames_remaining = 0;
    self.wrote_frames = 0;
    Ok(())
  }

  fn duration(&self) -> crate::Timecode {
    self.duration
  }

  fn builder(self: Box<Self>) -> crate::chain::AudioChainBuilder {
    AudioChainBuilder::new(self)
  }

  fn info(&self) -> AudioInfo {
    AudioInfo {
      num_channels: self.num_channels,
      sample_rate: self.sample_rate,
    }
  }
}

#[test]
fn test_symphonia_source_silence() -> Result<(), Error> {
  let proj_path = std::path::PathBuf::from("test/data/silence.flac");
  let mut source = SymphoniaAudioSource::new(proj_path)?;
  let mut buf = crate::buffer::PlanarVecBuffer::new(1, 44100);
  let samples = source.read(&mut buf)?;
  assert!(samples == 4410);
  assert!(buf.channel(0).iter().all(|f| *f == 0.0));
  Ok(())
}

#[test]
fn test_symphonia_source_sine() -> Result<(), Error> {
  let proj_path = std::path::PathBuf::from("test/data/sine.flac");
  let mut source = SymphoniaAudioSource::new(proj_path)?;
  let mut buf = crate::buffer::PlanarVecBuffer::new(1, 44100);
  let samples = source.read(&mut buf)?;
  assert!(samples == 4410);
  let tincr = 2.0 * std::f32::consts::PI * 1000.0 / 44100.0;
  for i in 0..4410 {
    let predicted = (i as f32 * tincr).sin();
    let actual = buf.channel(0)[i];
    assert!((actual.abs() - predicted.abs()).abs() < 0.001);
  }
  Ok(())
}

#[test]
fn test_symphonia_source_sine_stereo() -> Result<(), Error> {
  let proj_path = std::path::PathBuf::from("test/data/sine_stereo.flac");
  let mut source = SymphoniaAudioSource::new(proj_path)?;
  let mut buf = crate::buffer::PlanarVecBuffer::new(2, 44100);
  let frames = source.read(&mut buf)?;
  assert!(frames == 4410);
  let tincr = 2.0 * std::f32::consts::PI * 1000.0 / 44100.0;
  for i in 0..4410 {
    let predicted = (i as f32 * tincr).sin();
    let actual = buf.channel(0)[i];
    let actual2 = buf.channel(1)[i];
    assert!((actual.abs() - predicted.abs()).abs() < 0.01);
    assert!((actual2.abs() - predicted.abs()).abs() < 0.01);
  }
  Ok(())
}

#[test]
fn test_symphonia_source_sine_stereo_10s() -> Result<(), Error> {
  let proj_path = std::path::PathBuf::from("test/data/sine_stereo_10s.flac");
  let mut source = SymphoniaAudioSource::new(proj_path)?;
  let mut buf = crate::buffer::PlanarVecBuffer::new(2, 44100);
  let mut n = 0;
  for _ in 0..10 {
    let frames = source.read(&mut buf)?;
    assert!(frames == 44100);
    let tincr = 2.0 * std::f32::consts::PI * 1000.0 / 44100.0;
    for i in 0..44100 {
      let predicted = ((n + i) as f32 * tincr).sin();
      let actual = buf.channel(0)[i];
      let actual2 = buf.channel(1)[i];
      assert!((actual.abs() - predicted.abs()).abs() < 0.01);
      assert!((actual2.abs() - predicted.abs()).abs() < 0.01);
    }

    n += 44100;
  }
  let frames = source.read(&mut buf)?;
  assert!(frames == 0);
  Ok(())
}

#[test]
fn test_symphonia_source_sine_stereo_10s_mp3() -> Result<(), Error> {
  let proj_path = std::path::PathBuf::from("test/data/sine_stereo_10s.mp3");
  let mut source = SymphoniaAudioSource::new(proj_path)?;
  let mut buf = crate::buffer::PlanarVecBuffer::new(2, 1024);
  let mut n = 0;
  loop {
    let frames = source.read(&mut buf)?;
    let tincr = 2.0 * std::f32::consts::PI * 1000.0 / 44100.0;
    for i in 0..frames {
      let predicted = ((n + i) as f32 * tincr).sin();
      let actual = buf.channel(0)[i];
      let actual2 = buf.channel(1)[i];
      if (actual.abs() - predicted.abs()).abs() > 0.01 {
        println!("{} {} {} {}", n + i, predicted, actual, actual2);
        break;
      }
      assert!((actual.abs() - predicted.abs()).abs() < 0.01);
      assert!((actual2.abs() - predicted.abs()).abs() < 0.01);
    }

    n += frames;
    if frames < 1024 {
      break;
    }
  }
  let frames = source.read(&mut buf)?;
  assert!(frames == 0);
  Ok(())
}
