use std::path::Path;

use ffmpeg_next::{Rational, Rescale, decoder, format::context::StreamIo, frame, rescale};

use crate::{
  AudioSource, Error, PlanarAudioBuffer, Timecode, chain::AudioChainBuilder, chain::AudioInfo,
};

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
  same_frame: bool,
}

impl FfmpegAudioSource {
  pub fn new<P: AsRef<Path>>(path: P) -> Result<FfmpegAudioSource, Error> {
    let stream = std::fs::File::open(path.as_ref())?;
    let filename = path.as_ref().file_name().and_then(|f| f.to_str());
    let input = ffmpeg_next::format::input_from_stream(
      StreamIo::from_read_seek(stream)?,
      filename,
      Option::None,
    )?;

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
      same_frame: in_frame.format() == out_frame.format(),
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
      let frame = &self.out_frame;
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
        let frame = if self.same_frame {
          // You'd think we'd be able to read directly from in_frame but for
          // some reason it's dead by the next time the function is called, for
          // use with remaining_frames. Not really sure why this is so
          // let's just add an extra memcpy to make sure it doesn't
          // happen.
          let frames = self.in_frame.num_frames();
          self.out_frame.set_samples(frames);
          self
            .out_frame
            .copy_from_buffer(&self.in_frame, 0, 0, frames);
          &mut self.out_frame
        } else {
          let delay = self.sample_conv.run(&self.in_frame, &mut self.out_frame)?;
          assert!(delay.is_none());
          &mut self.out_frame
        };

        assert!(self.decoder.delay() == 0);

        let num = (buffer.num_frames() - frames_written).min(frame.samples());
        buffer.copy_from_buffer(frame, 0, frames_written, num);
        frames_written += num;
        self.remaining_frames = frame.samples() - num;
      }
    }
  }

  fn seek(&mut self, pos: Timecode) -> Result<(), Error> {
    let time = (pos.to_seconds_f64() * self.sample_rate as f64) as i64;
    let time = time.rescale(
      Rational::new(1, self.sample_rate as i32),
      rescale::TIME_BASE,
    );
    self.input.seek(time, ..time)?;
    Ok(())
  }

  fn duration(&self) -> Timecode {
    let duration = self.input.duration() as usize;
    Timecode {
      sample_rate: self.time_base.1 as usize,
      samples: duration * self.time_base.0 as usize,
    }
  }

  fn builder(self: Box<Self>) -> AudioChainBuilder {
    AudioChainBuilder::new(self)
  }

  fn info(&self) -> crate::chain::AudioInfo {
    AudioInfo {
      num_channels: self.num_channels,
      sample_rate: self.sample_rate,
    }
  }
}

#[test]
fn test_ffmpeg_source_silence() -> Result<(), Error> {
  let proj_path = std::path::PathBuf::from("test/data/silence.flac");
  let mut source = FfmpegAudioSource::new(proj_path)?;
  let mut buf = crate::buffer::PlanarVecBuffer::new(1, 44100);
  let samples = source.read(&mut buf)?;
  assert!(samples == 4410);
  assert!(buf.channel(0).iter().all(|f| *f == 0.0));
  Ok(())
}

#[test]
fn test_ffmpeg_source_sine() -> Result<(), Error> {
  let proj_path = std::path::PathBuf::from("test/data/sine.flac");
  let mut source = FfmpegAudioSource::new(proj_path)?;
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
fn test_ffmpeg_source_sine_stereo() -> Result<(), Error> {
  let proj_path = std::path::PathBuf::from("test/data/sine_stereo.flac");
  let mut source = FfmpegAudioSource::new(proj_path)?;
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
fn test_ffmpeg_source_sine_stereo_10s() -> Result<(), Error> {
  let proj_path = std::path::PathBuf::from("test/data/sine_stereo_10s.flac");
  let mut source = FfmpegAudioSource::new(proj_path)?;
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
fn test_ffmpeg_source_sine_stereo_10s_mp3() -> Result<(), Error> {
  let proj_path = std::path::PathBuf::from("test/data/sine_stereo_10s.mp3");
  let mut source = FfmpegAudioSource::new(proj_path)?;
  let mut buf = crate::buffer::PlanarVecBuffer::new(2, 1024);
  let mut n = 0;
  loop {
    let frames = source.read(&mut buf)?;
    let tincr = 2.0 * std::f32::consts::PI * 1000.0 / 44100.0;
    for i in 0..frames {
      let predicted = ((n + i) as f32 * tincr).sin();
      let actual = buf.channel(0)[i];
      let actual2 = buf.channel(1)[i];
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
