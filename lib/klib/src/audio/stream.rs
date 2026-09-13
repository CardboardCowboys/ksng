use audioadapter_buffers::number_to_float::SequentialNumbers;
use creek::{ReadDiskStream, SymphoniaDecoder};
use rubato::Resampler;

use crate::error::Error;

pub trait AudioStream {
  fn read(&mut self, buffer_out: &mut [Vec<f32>]) -> Result<usize, Error>;
  fn seek(&mut self, to_frame: usize);
}

struct RawAudioStream {
  out_buffer_size: usize,
  read_stream: ReadDiskStream<SymphoniaDecoder>,
}

impl AudioStream for RawAudioStream {
  fn read(&mut self, buffer_out: &mut [Vec<f32>]) -> Result<usize, Error> {
    let data = self
      .read_stream
      .read(self.out_buffer_size)
      .map_err(|e| crate::error::Error::Audio(e.to_string()))?;

    for (i, ch) in buffer_out.iter_mut().enumerate().take(data.num_channels()) {
      ch.copy_from_slice(data.read_channel(i));
    }

    Ok(data.num_frames())
  }

  fn seek(&mut self, to_frame: usize) {
    let _ = self.read_stream.seek(to_frame, creek::SeekMode::Auto);
  }
}

struct ResampledAudioStream {
  out_buffer_size: usize,
  raw_stream: RawAudioStream,
  resampler: rubato::Fft<f32>,
  input_buffers: Vec<Vec<f32>>,
  resample_buffers: Vec<Vec<f32>>,
  input_block_size: usize,
  output_block_size: usize,
  remaining_samples: usize,
  remaining_offset: usize,
  channels: usize,
}

impl AudioStream for ResampledAudioStream {
  fn read(&mut self, buffer_out: &mut [Vec<f32>]) -> Result<usize, Error> {
    let out_offset = if self.remaining_samples > 0 {
      // There are samples remaining from last time we resampled a stream - copy
      // those over before overwriting the buffers.
      for (i, ch) in buffer_out.iter_mut().enumerate().take(self.channels) {
        ch.copy_from_slice(
          &self.resample_buffers[i]
            [self.remaining_offset..(self.remaining_offset + self.remaining_samples)],
        );
      }

      let was_written = self.remaining_samples;

      if self.remaining_samples <= self.out_buffer_size {
        self.remaining_offset = 0;
        self.remaining_samples = 0;
      } else {
        self.remaining_offset += self.out_buffer_size;
        self.remaining_samples -= self.out_buffer_size;
      }

      was_written
    } else {
      0
    };

    // We had enough samples left over to fully fill the output buffer.
    if out_offset >= self.out_buffer_size {
      return Ok(self.out_buffer_size);
    }

    self.raw_stream.read(&mut self.input_buffers)?;
    for i in 0..self.channels {
      self.resample_buffers[i].fill(0.0_f32);
      let (_, output_frames) = self
        .resampler
        .process_into_buffer(
          &SequentialNumbers::new(&self.input_buffers[i], 1, self.input_block_size).unwrap(),
          &mut SequentialNumbers::new_mut(&mut self.resample_buffers[i], 1, self.output_block_size)
            .unwrap(),
          None,
        )
        .map_err(|e| crate::error::Error::Audio(format!("Rubato error: {e:?}")))?;
      assert!(self.output_block_size == output_frames);
    }

    for (i, ch) in buffer_out.iter_mut().enumerate().take(self.channels) {
      ch.copy_from_slice(&self.resample_buffers[i][0..self.out_buffer_size]);
    }

    self.remaining_offset = self.out_buffer_size;
    self.remaining_samples = self.output_block_size.saturating_sub(self.out_buffer_size);

    Ok(self.out_buffer_size)
  }

  fn seek(&mut self, to_frame: usize) {
    self.raw_stream.seek(to_frame)
  }
}

pub fn new_stream(
  stream: ReadDiskStream<SymphoniaDecoder>,
  out_buffer_size: usize,
  out_sample_rate: usize,
) -> Result<Box<dyn AudioStream + Send>, Error> {
  let sample_rate = stream.info().sample_rate.ok_or(crate::error::Error::Audio(
    "Tried to load audio file without sample rate".into(),
  ))? as usize;

  if sample_rate == out_sample_rate {
    return Ok(Box::new(RawAudioStream {
      out_buffer_size,
      read_stream: stream,
    }));
  }

  let resampler = rubato::Fft::<f32>::new(
    sample_rate,
    out_sample_rate,
    out_buffer_size,
    1,
    1,
    rubato::FixedSync::Both,
  )
  .map_err(|e| crate::error::Error::Audio(e.to_string()))?;

  let input_block_size = resampler.input_frames_max();
  let output_block_size = resampler.output_frames_max();
  let num_channels = stream.info().num_channels.max(2) as usize;

  let mut input_buffers = Vec::with_capacity(num_channels);
  for _ in 0..num_channels {
    let mut buffer = Vec::with_capacity(input_block_size);
    buffer.resize(input_block_size, 0.0_f32);
    input_buffers.push(buffer);
  }

  let mut resample_buffers = Vec::with_capacity(num_channels);
  for _ in 0..num_channels {
    let mut buffer = Vec::with_capacity(output_block_size);
    buffer.resize(output_block_size, 0.0_f32);
    resample_buffers.push(buffer);
  }

  Ok(Box::new(ResampledAudioStream {
    out_buffer_size,
    raw_stream: RawAudioStream {
      out_buffer_size,
      read_stream: stream,
    },
    resampler,
    input_buffers,
    resample_buffers,
    input_block_size,
    output_block_size,
    remaining_samples: 0,
    remaining_offset: 0,
    channels: num_channels,
  }))
}
