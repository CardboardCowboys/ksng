use rubato::Resampler;

use crate::{
  AudioFilter, Error, PlanarAudioBuffer,
  buffer::PlanarVecBuffer,
  chain::{AudioChainBuilder, AudioChainWalker, AudioInfo},
};

struct ResamplerFilter {
  resampler: rubato::Fft<f32>,
  in_buffer: PlanarVecBuffer,
  out_buffer: PlanarVecBuffer,
  remaining_out_frames: usize,
  out_sample_rate: usize,
  num_channels: usize,
}

impl AudioFilter for ResamplerFilter {
  fn read(
    &mut self,
    buffer: &mut dyn crate::PlanarAudioBuffer,
    mut ancestor: AudioChainWalker,
  ) -> Result<usize, Error> {
    let mut frames_written = 0;
    if self.remaining_out_frames > 0 {
      let num = self.remaining_out_frames.min(buffer.num_frames());
      buffer.copy_from_buffer(
        &self.out_buffer,
        self.out_buffer.num_frames() - self.remaining_out_frames,
        0,
        num,
      );
      frames_written += num;
      self.remaining_out_frames -= num;
    }

    loop {
      if frames_written >= buffer.num_frames() {
        return Ok(frames_written);
      }

      self.in_buffer.fill(0.0_f32);
      let _num_read = ancestor.read(&mut self.in_buffer)?;
      let in_adapter = self.in_buffer.into_adapter()?;
      let mut out_adapter = self.out_buffer.into_adapter_mut()?;
      let (in_frames, out_frames) =
        self
          .resampler
          .process_into_buffer(&in_adapter, &mut out_adapter, None)?;
      assert!(in_frames == 0); // should be true if we did everything right

      let num_to_write = (buffer.num_frames() - frames_written).min(out_frames);
      buffer.copy_from_buffer(&self.out_buffer, 0, frames_written, num_to_write);
      self.remaining_out_frames = out_frames - num_to_write;
    }
  }

  fn seek(&mut self, pos: crate::Timecode, mut ancestor: AudioChainWalker) -> Result<(), Error> {
    self.remaining_out_frames = 0;
    self.resampler.reset();
    ancestor.seek(pos)
  }

  fn duration(&self, ancestor: AudioChainWalker) -> crate::Timecode {
    ancestor.duration()
  }

  fn info(&self) -> AudioInfo {
    AudioInfo {
      num_channels: self.num_channels,
      sample_rate: self.out_sample_rate,
    }
  }
}

pub trait WithResamplerFilter {
  /// Adds a `ResamplerFilter` to the chain, changing the sample rate.
  fn with_resampler(self, target_sample_rate: usize) -> Result<AudioChainBuilder, Error>;
}

impl WithResamplerFilter for AudioChainBuilder {
  fn with_resampler(mut self, target_sample_rate: usize) -> Result<AudioChainBuilder, Error> {
    let info = self.info();

    let resampler = rubato::Fft::<f32>::new(
      info.sample_rate,
      target_sample_rate,
      1024,
      info.num_channels,
      rubato::FixedSync::Both,
    )?;

    let in_buffer_size = resampler.input_frames_max();
    let in_buffer = PlanarVecBuffer::new(info.num_channels, in_buffer_size);

    let out_buffer_size = resampler.output_frames_max();
    let out_buffer = PlanarVecBuffer::new(info.num_channels, out_buffer_size);

    let filter = ResamplerFilter {
      resampler,
      in_buffer,
      out_buffer,
      remaining_out_frames: 0,
      out_sample_rate: target_sample_rate,
      num_channels: info.num_channels,
    };

    self.add_filter(filter);

    Ok(self)
  }
}
