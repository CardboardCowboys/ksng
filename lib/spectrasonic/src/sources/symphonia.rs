use std::{io::ErrorKind, path::Path};

use infer::Infer;
use symphonia::core::{
  audio::{AudioBuffer, Signal, SignalSpec},
  codecs::{Decoder, DecoderOptions},
  formats::{FormatOptions, FormatReader},
  io::MediaSourceStream,
  meta::MetadataOptions,
  probe::Hint,
  units::Time,
};

use crate::{
  AudioChainBuilder, AudioInfo, AudioSource, Error, Timecode, buffer::PlanarAudioBuffer,
};

fn timecode_to_time(time: Timecode) -> Time {
  let seconds = time.to_seconds_f64();
  Time {
    seconds: seconds as u64,
    frac: seconds.fract(),
  }
}

impl PlanarAudioBuffer for AudioBuffer<f32> {
  fn num_frames(&self) -> usize {
    self.capacity()
  }

  fn num_channels(&self) -> usize {
    self.spec().channels.count()
  }

  fn channel(&self, i: usize) -> &[f32] {
    self.chan(i)
  }

  fn channel_mut(&mut self, i: usize) -> &mut [f32] {
    self.chan_mut(i)
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
  buffer: AudioBuffer<f32>,
  frames_remaining: usize,
  sample_rate: usize,
  num_channels: usize,
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
    let duration = Timecode {
      samples: n_frames as usize,
      sample_rate: sample_rate as usize,
    };
    let decoder = codecs.make(&track.codec_params, &decoder_opts)?;

    let buffer_size = if let Some(max_size) = track.codec_params.max_frames_per_packet {
      max_size * 2
    } else {
      sample_rate as u64 * 2
    };
    let buffer = AudioBuffer::new(buffer_size, SignalSpec::new(sample_rate, channels));

    let track_id = track.id;
    let required_offset = track.codec_params.delay.unwrap_or(0) as usize;

    Ok(SymphoniaAudioSource {
      decoder,
      format,
      track_id,
      duration,
      required_offset,
      buffer,
      frames_remaining: 0,
      sample_rate: sample_rate as usize,
      num_channels: channels.count(),
    })
  }
}

impl AudioSource for SymphoniaAudioSource {
  fn read(&mut self, buffer: &mut dyn crate::buffer::PlanarAudioBuffer) -> Result<usize, Error> {
    let mut frames_written = 0;

    if self.frames_remaining > 0 {
      let num = self.frames_remaining.min(buffer.num_frames());
      buffer.copy_from_buffer(
        &self.buffer,
        self.buffer.num_frames() - self.frames_remaining,
        0,
        num,
      );
      self.frames_remaining -= num;
      frames_written += num;
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
        Err(e) => return Err(e.into()),
      };

      if packet.track_id() != self.track_id {
        continue;
      }

      let audio_buf = self.decoder.decode(&packet)?;
      audio_buf.convert(&mut self.buffer);

      if self.required_offset >= self.buffer.frames() {
        self.required_offset -= self.buffer.frames();
      } else if self.required_offset > 0 {
        let num =
          (self.buffer.frames() - self.required_offset).min(buffer.num_frames() - frames_written);
        buffer.copy_from_buffer(&self.buffer, self.required_offset, frames_written, num);
        self.required_offset = 0;
        frames_written += num;
        self.frames_remaining = self.buffer.frames() - num;
      } else {
        let num = self
          .buffer
          .frames()
          .min(buffer.num_frames() - frames_written);
        buffer.copy_from_buffer(&self.buffer, 0, frames_written, num);
        frames_written += num;
        self.frames_remaining = self.buffer.frames() - num;
      }
    }
  }

  fn seek(&mut self, pos: crate::Timecode) -> Result<(), Error> {
    let seeked_to = self.format.seek(
      symphonia::core::formats::SeekMode::Accurate,
      symphonia::core::formats::SeekTo::Time {
        time: timecode_to_time(pos),
        track_id: None,
      },
    )?;
    self.required_offset = (seeked_to.required_ts - seeked_to.actual_ts) as usize;
    // Force re-read instead of using cached data
    self.frames_remaining = 0;
    Ok(())
  }

  fn duration(&self) -> crate::Timecode {
    self.duration
  }

  fn builder(self) -> crate::AudioChainBuilder {
    AudioChainBuilder::new(self)
  }

  fn info(&self) -> AudioInfo {
    AudioInfo {
      num_channels: self.num_channels,
      sample_rate: self.sample_rate,
    }
  }
}
