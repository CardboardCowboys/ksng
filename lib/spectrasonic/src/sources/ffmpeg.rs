use std::path::Path;

use ffmpeg_next::{Rational, decoder, frame};

use crate::{AudioChainBuilder, AudioInfo, AudioSource, Error, PlanarAudioBuffer, Timecode};

impl PlanarAudioBuffer for frame::Audio {
  fn num_frames(&self) -> usize {
    self.samples()
  }

  fn num_channels(&self) -> usize {
    self.channels() as usize
  }

  fn channel(&self, i: usize) -> &[f32] {
    self.plane(i)
  }

  fn channel_mut(&mut self, i: usize) -> &mut [f32] {
    self.plane_mut(i)
  }

  fn fill(&mut self, v: f32) {
    for ch in 0..self.channels() {
      self.plane_mut(ch as usize).fill(v);
    }
  }
}

pub struct FfmpegAudioSource {
  decoder: decoder::Audio,
  input: ffmpeg_next::format::context::Input,
  time_base: Rational,
  num_channels: usize,
  sample_rate: usize,
  remaining_frames: usize,
  stream_index: usize,
  in_frame: frame::Audio,
  out_frame: frame::Audio,
  sample_conv: ffmpeg_next::software::resampling::Context,
}

impl FfmpegAudioSource {
  pub fn new<P: AsRef<Path>>(path: P) -> Result<FfmpegAudioSource, Error> {
    let input = ffmpeg_next::format::input(&path)?;

    let in_stream = input
      .streams()
      .best(ffmpeg_next::media::Type::Audio)
      .ok_or(Error::msg(format!(
        "Could not find audio stream in {:?}",
        path.as_ref()
      )))?;

    let context = ffmpeg_next::codec::context::Context::from_parameters(in_stream.parameters())?;
    let decoder = context.decoder().audio()?;
    let time_base = in_stream.time_base();

    let num_channels = decoder.channels() as usize;
    let sample_rate = decoder.rate() as usize;
    let stream_index = in_stream.index();

    let in_frame = frame::Audio::new(
      decoder.format(),
      decoder.frame_size() as usize,
      decoder.channel_layout(),
    );
    let out_frame = frame::Audio::new(
      ffmpeg_next::format::Sample::F32(ffmpeg_next::format::sample::Type::Planar),
      decoder.frame_size() as usize,
      decoder.channel_layout(),
    );

    let sample_conv = ffmpeg_next::software::resampler(
      (
        in_frame.format(),
        in_frame.channel_layout(),
        sample_rate as u32,
      ),
      (
        out_frame.format(),
        out_frame.channel_layout(),
        sample_rate as u32,
      ),
    )?;

    Ok(FfmpegAudioSource {
      decoder,
      input,
      time_base,
      num_channels,
      sample_rate,
      remaining_frames: 0,
      stream_index,
      in_frame,
      out_frame,
      sample_conv,
    })
  }
}

impl AudioSource for FfmpegAudioSource {
  fn read(&mut self, buffer: &mut dyn crate::PlanarAudioBuffer) -> Result<usize, Error> {
    let mut frames_written = 0;
    if self.remaining_frames > 0 {
      let num = self.remaining_frames.min(buffer.num_frames());
      let frame = if self.in_frame.format() == self.out_frame.format() {
        &self.in_frame
      } else {
        &self.out_frame
      };
      buffer.copy_from_buffer(frame, frame.samples() - self.remaining_frames, 0, num);
      frames_written += num;
      self.remaining_frames -= num;
    }

    loop {
      if frames_written >= buffer.num_frames() {
        return Ok(frames_written);
      }

      let Some((stream, packet)) = self.input.packets().next() else {
        return Ok(frames_written);
      };

      if stream.index() != self.stream_index {
        continue;
      }

      self.decoder.send_packet(&packet)?;
      while self.decoder.receive_frame(&mut self.in_frame).is_ok() {
        let frame = if self.in_frame.format() == self.out_frame.format() {
          &mut self.in_frame
        } else {
          let delay = self.sample_conv.run(&self.in_frame, &mut self.out_frame)?;
          assert!(delay.is_none());
          &mut self.out_frame
        };

        let num = (buffer.num_frames() - frames_written).min(frame.samples());
        buffer.copy_from_buffer(frame, 0, frames_written, num);
        frames_written += num;
        self.remaining_frames = frame.samples() - num;
      }
    }
  }

  fn seek(&mut self, pos: Timecode) -> Result<(), Error> {
    let inv_ratio = self.time_base.1 as f64 / self.time_base.0 as f64;
    let time = (pos.to_seconds_f64() * inv_ratio) as i64;
    let ofs = (0.1 * inv_ratio) as i64;
    self.input.seek(time, (time - ofs)..(time + ofs))?;
    Ok(())
  }

  fn duration(&self) -> Timecode {
    let duration = self.input.duration() as usize;
    Timecode {
      sample_rate: self.time_base.1 as usize,
      samples: duration * self.time_base.0 as usize,
    }
  }

  fn builder(self) -> AudioChainBuilder {
    AudioChainBuilder::new(self)
  }

  fn info(&self) -> crate::AudioInfo {
    AudioInfo {
      num_channels: self.num_channels,
      sample_rate: self.sample_rate,
    }
  }
}
