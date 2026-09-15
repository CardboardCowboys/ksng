use circular_buffer::FixedCircularBuffer;

use crate::error::Error;

pub mod info;
pub mod mixer_stream;
pub mod stream;

/// Trait for audio objects that produce interleaved 2-channel f32 samples.
pub trait SampleProducer {
  /// Fills `buffer` with up to `block_size()` samples per channel, returning
  /// the number of samples written.
  fn process(&mut self, buffer: &mut [f32]) -> Result<usize, Error>;
  /// The maximum number of frames produced by a call to `process`.
  /// The buffer passed to `process` should be at least twice this size.
  fn block_size(&self) -> usize;
}

/// Takes audio sample blocks from the producer and produces blocks of a given
/// size, regardless of the block size of the producer.
/// N = size of the internal buffer in samples (not frames!). Must be large
/// enough to hold the larger of (to_size, producer.block_size).
pub struct Reblocker<'producer, Producer: SampleProducer + 'producer, const N: usize> {
  to_size: usize,
  buffer: FixedCircularBuffer<f32, N>,
  scratch_buffer: Vec<f32>,
  producer: &'producer mut Producer,
}

impl<'producer, Producer: SampleProducer, const N: usize> Reblocker<'producer, Producer, N> {
  pub fn new(
    producer: &'producer mut Producer,
    to_size: usize,
  ) -> Reblocker<'producer, Producer, N> {
    assert!(producer.block_size() <= N && to_size <= N);
    let mut scratch_buffer = Vec::with_capacity(producer.block_size() * 2);
    scratch_buffer.resize(producer.block_size() * 2, 0.0_f32);
    Reblocker {
      to_size,
      buffer: Default::default(),
      producer,
      scratch_buffer,
    }
  }
}

fn write_from_slices(in_slices: (&[f32], &[f32]), out_buffer: &mut [f32], num: usize) -> usize {
  let (a, b) = in_slices;
  let num_from_a = a.len().min(num);
  out_buffer[0..num_from_a].copy_from_slice(&a[0..num_from_a]);
  if num_from_a < num {
    let num_from_b = (num - num_from_a).min(b.len());
    out_buffer[num_from_a..(num_from_a + num_from_b)].copy_from_slice(&b[0..num_from_b]);
    num_from_a + num_from_b
  } else {
    num_from_a
  }
}

impl<'producer, Producer: SampleProducer + 'producer, const N: usize> SampleProducer
  for Reblocker<'producer, Producer, N>
{
  fn process(&mut self, out_buffer: &mut [f32]) -> Result<usize, Error> {
    let mut num_samples = self.to_size * 2;
    // Start by copying all remaining samples
    let num_written = write_from_slices(self.buffer.as_slices(), out_buffer, num_samples);
    if num_written >= num_samples {
      return Ok(num_written);
    }

    num_samples -= num_written;
    while num_samples > 0 {
      let samples_produced = self.producer.process(&mut self.scratch_buffer)?;
      self
        .buffer
        .extend_from_slice(&self.scratch_buffer[0..samples_produced]);
      let to_write = num_samples.min(samples_produced);
      write_from_slices(
        self.buffer.as_slices(),
        &mut out_buffer[num_written..(num_written + to_write)],
        to_write,
      );
      num_samples -= to_write;
      if samples_produced < self.scratch_buffer.len() {
        // No more samples will be produced, give up now.
        break;
      }
    }

    Ok((self.to_size * 2) - num_samples)
  }

  fn block_size(&self) -> usize {
    self.to_size
  }
}
