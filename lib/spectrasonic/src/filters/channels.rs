use crate::{AudioChainBuilder, AudioFilter, AudioInfo, Error, PlanarAudioBuffer, PlanarVecBuffer};

const BLOCK_SIZE: usize = 1024;

struct ChannelRemapperFilter {
  to_channels: usize,
  sample_rate: usize,
  buffer: PlanarVecBuffer,
  remaining_frames: usize,
  mapping: Vec<usize>,
}

impl ChannelRemapperFilter {
  fn remap_and_write(
    &self,
    buffer: &mut dyn PlanarAudioBuffer,
    num: usize,
    from_offset: usize,
    to_offset: usize,
  ) {
    for (to_idx, from_idx) in self.mapping.iter().copied().enumerate() {
      buffer.channel_mut(to_idx)[to_offset..(to_offset + num)]
        .copy_from_slice(&self.buffer.channel(from_idx)[from_offset..(from_offset + num)]);
    }
  }
}

impl AudioFilter for ChannelRemapperFilter {
  fn read(
    &mut self,
    buffer: &mut dyn crate::PlanarAudioBuffer,
    mut ancestor: crate::AudioChainWalker,
  ) -> Result<usize, Error> {
    assert!(buffer.num_channels() == self.to_channels);
    let mut frames_written = 0;
    if self.remaining_frames > 0 {
      let num_to_write = self.remaining_frames.min(buffer.num_frames());
      self.remap_and_write(
        buffer,
        num_to_write,
        self.buffer.num_frames() - self.remaining_frames,
        0,
      );
      frames_written += num_to_write;
      self.remaining_frames -= num_to_write;
    }

    loop {
      if frames_written >= buffer.num_frames() {
        return Ok(frames_written);
      }

      let num_read = ancestor.read(&mut self.buffer)?;
      let num_to_write = (buffer.num_frames() - frames_written).min(num_read);
      self.remap_and_write(buffer, num_to_write, 0, frames_written);
      frames_written += num_to_write;
      self.remaining_frames = num_read - num_to_write;
    }
  }

  fn seek(
    &mut self,
    pos: crate::Timecode,
    mut ancestor: crate::AudioChainWalker,
  ) -> Result<(), Error> {
    self.remaining_frames = 0;
    ancestor.seek(pos)
  }

  fn duration(&self, ancestor: crate::AudioChainWalker) -> crate::Timecode {
    ancestor.duration()
  }

  fn info(&self) -> AudioInfo {
    AudioInfo {
      num_channels: self.to_channels,
      sample_rate: self.sample_rate,
    }
  }
}

pub trait WithChannelRemapperFilter {
  /// Adds a `ChannelRemapperFilter` to the chain, remapping the number of
  /// channels to `new_channels`.
  ///
  /// `mapping` specifies how the channels will be remapped. Each index
  /// represents one of the new channels, and the value represents the index of
  /// the old channel.
  ///
  /// For example, a mapping of [0, 0] will copy a mono source to a stereo
  /// output.
  fn with_channel_remapper(
    self,
    new_channels: usize,
    mapping: &[usize],
  ) -> Result<AudioChainBuilder, Error>;
}

impl WithChannelRemapperFilter for AudioChainBuilder {
  fn with_channel_remapper(
    mut self,
    new_channels: usize,
    mapping: &[usize],
  ) -> Result<AudioChainBuilder, Error> {
    if new_channels < 1 {
      return Err(Error::msg(
        "Can't create a channel remapper with 0 channels",
      ));
    }

    if mapping.len() != new_channels {
      return Err(Error::msg(format!(
        "Created a channel remapper with {new_channels} channels but a mapping of len {} was given",
        mapping.len()
      )));
    }

    let info = self.info();

    for i in mapping {
      if *i >= info.num_channels {
        return Err(Error::msg(format!(
          "Ancestor only provides {} channels but mapping contains index {i}",
          info.num_channels
        )));
      }
    }

    let buffer = PlanarVecBuffer::new(info.num_channels, BLOCK_SIZE);

    let filter = ChannelRemapperFilter {
      sample_rate: info.sample_rate,
      to_channels: new_channels,
      mapping: mapping.to_vec(),
      buffer,
      remaining_frames: 0,
    };

    self.add_filter(filter);

    Ok(self)
  }
}
