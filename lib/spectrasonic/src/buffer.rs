use audioadapter_buffers::direct::SequentialSlice;

pub trait PlanarAudioBuffer {
  /// Returns the number of frames this buffer can hold.
  fn num_frames(&self) -> usize;
  /// Returns the number of channels this buffer can hold.
  fn num_channels(&self) -> usize;
  /// Returns a slice to the samples of the channel at `i` index.
  fn channel(&self, i: usize) -> &[f32];
  /// Returns a mutable slice to the samples of the channel at `i` index.
  fn channel_mut(&mut self, i: usize) -> &mut [f32];
  /// Copies the range `from_offset..(from_offset + count)` from the provided
  /// buffer to this buffer in the range `to_offset..(to_offset + count)`. The
  /// number of channels in both buffers must match and the ranges must be
  /// within both buffers.
  fn copy_from_buffer(
    &mut self,
    buffer: &dyn PlanarAudioBuffer,
    from_offset: usize,
    to_offset: usize,
    count: usize,
  ) {
    if count == 0 {
      return;
    }
    assert!(buffer.num_channels() == self.num_channels());
    assert!(from_offset < buffer.num_frames() && (from_offset + count) <= buffer.num_frames());
    assert!(to_offset < self.num_frames() && (to_offset + count) <= self.num_frames());
    for i in 0..self.num_channels() {
      self.channel_mut(i)[to_offset..(to_offset + count)]
        .copy_from_slice(&buffer.channel(i)[from_offset..(from_offset + count)]);
    }
  }
  /// Fills all channels with the value `v`.
  fn fill(&mut self, v: f32);
}

pub struct PlanarSliceBuffer<'slice>(usize, &'slice mut [f32]);

impl<'slice> PlanarAudioBuffer for PlanarSliceBuffer<'slice> {
  fn num_frames(&self) -> usize {
    self.1.len() / self.0
  }

  fn num_channels(&self) -> usize {
    self.0
  }

  fn channel(&self, i: usize) -> &[f32] {
    assert!(i < self.0);
    let channel_size = self.num_frames();
    &self.1[(channel_size * i)..(channel_size * (i + 1))]
  }

  fn channel_mut(&mut self, i: usize) -> &mut [f32] {
    assert!(i < self.0);
    let channel_size = self.num_frames();
    &mut self.1[(channel_size * i)..(channel_size * (i + 1))]
  }

  fn fill(&mut self, v: f32) {
    self.1.fill(v)
  }
}

pub struct PlanarVecBuffer(usize, Vec<f32>);

impl PlanarVecBuffer {
  pub fn new(num_channels: usize, num_frames: usize) -> PlanarVecBuffer {
    let size = num_channels * num_frames;
    let mut buffer = Vec::with_capacity(size);
    buffer.resize(size, 0.0_f32);
    PlanarVecBuffer(num_channels, buffer)
  }

  pub fn into_adapter(&self) -> Result<SequentialSlice<&[f32]>, audioadapter_buffers::SizeError> {
    SequentialSlice::new(&self.1, self.0, self.num_frames())
  }

  pub fn into_adapter_mut(
    &mut self,
  ) -> Result<SequentialSlice<&mut [f32]>, audioadapter_buffers::SizeError> {
    let num_frames = self.num_frames();
    SequentialSlice::new_mut(&mut self.1, self.0, num_frames)
  }
}

impl PlanarAudioBuffer for PlanarVecBuffer {
  fn num_frames(&self) -> usize {
    self.1.len() / self.0
  }

  fn num_channels(&self) -> usize {
    self.0
  }

  fn channel(&self, i: usize) -> &[f32] {
    assert!(i < self.0);
    let channel_size = self.num_frames();
    &self.1[(channel_size * i)..(channel_size * (i + 1))]
  }

  fn channel_mut(&mut self, i: usize) -> &mut [f32] {
    assert!(i < self.0);
    let channel_size = self.num_frames();
    &mut self.1[(channel_size * i)..(channel_size * (i + 1))]
  }

  fn fill(&mut self, v: f32) {
    self.1.fill(v)
  }
}

#[cfg(feature = "ffmpeg")]
impl PlanarAudioBuffer for ffmpeg_next::frame::Audio {
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

#[test]
pub fn test_slice_buffer() {
  let mut data = [0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
  let mut slice_buffer = PlanarSliceBuffer(2, &mut data);
  assert!(slice_buffer.num_frames() == 4);
  assert!(slice_buffer.num_channels() == 2);
  assert!(slice_buffer.channel(0).iter().all(|f| *f == 0.0));
  assert!(slice_buffer.channel(1).iter().all(|f| *f == 1.0));
  slice_buffer.fill(3.0);
  assert!(slice_buffer.channel(0).iter().all(|f| *f == 3.0));
  assert!(slice_buffer.channel(1).iter().all(|f| *f == 3.0));
}
