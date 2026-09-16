use std::{cell::RefCell, rc::Rc};

use crate::{Error, Timecode, buffer::PlanarAudioBuffer};

/// Contains info about an audio provider, whether source or filter.
#[derive(Clone, Copy)]
pub struct AudioInfo {
  /// How many channels of audio the source returns.
  pub num_channels: usize,
  /// The sample rate of the source.
  pub sample_rate: usize,
}

/// Defines an audio source that produces samples.
pub trait AudioSource: 'static {
  /// Reads samples from the source into `buffer`.
  /// The buffer can be any number of frames, but must match the number of
  /// channels of the source.
  ///
  /// The number of frames written into the buffer is returned. If the number of
  /// frames is less than the size of the buffer, it means the source has no
  /// more samples to send.
  fn read(&mut self, buffer: &mut dyn PlanarAudioBuffer) -> Result<usize, Error>;
  /// Seeks the source to a given `Timecode`.
  fn seek(&mut self, pos: Timecode) -> Result<(), Error>;
  /// Returns the duration of the source.
  fn duration(&self) -> Timecode;
  /// Creates an `AudioChainBuilder` from this source. If using this source with
  /// any number of filters, this should be used to chain those filters
  /// together.
  fn builder(self) -> AudioChainBuilder;
  /// Returns the `AudioInfo` of this source.
  fn info(&self) -> AudioInfo;
}

/// Defines a filter that takes samples from an ancestor source or filter and
/// performs some sort of processing on them.
pub trait AudioFilter {
  /// Writes processed samples into `buffer`. The buffer can be any number of
  /// frames long, but its channel count must match the filter's
  /// `info().num_channels`.
  ///
  /// The `ancestor` is an `AudioChainWalker` that allows the filter to access
  /// either the filter before it or the source without knowing exactly where it
  /// is in the chain.
  fn read(
    &mut self,
    buffer: &mut dyn PlanarAudioBuffer,
    ancestor: AudioChainWalker,
  ) -> Result<usize, Error>;
  /// Seeks the filter to a given `Timecode`.
  ///
  /// This will likely just mean that the seek operation is passed through to
  /// the ancestor, but this operation might also clear internal buffers or
  /// change the timecode passed up the chain.
  fn seek(&mut self, pos: Timecode, ancestor: AudioChainWalker) -> Result<(), Error>;
  /// Returns the duration of this filter.
  ///
  /// For most filters, this will be the duration of the source, but some
  /// filters may change the duration of the underlying source (for example, a
  /// time remapping filter).
  fn duration(&self, ancestor: AudioChainWalker) -> Timecode;
  /// Returns the `AudioInfo` of this filter.
  fn info(&self) -> AudioInfo;
}

struct AudioChainInner {
  source: RefCell<Box<dyn AudioSource>>,
  filters: Vec<RefCell<Box<dyn AudioFilter>>>,
}

/// An `AudioChain` represents a source along with optionally one or more
/// filters that transform the source.
///
/// An `AudioChain` can be built using the `AudioChainBuilder` through an
/// `AudioSource`'s `builder` method.
pub struct AudioChain {
  inner: Rc<AudioChainInner>,
  info: AudioInfo,
}

impl AudioChain {
  /// Writes processed samples into `buffer`. The buffer can be any number of
  /// frames long, but its channel count must match the chains's
  /// `info().num_channels`.
  pub fn read(&self, buffer: &mut dyn PlanarAudioBuffer) -> Result<usize, Error> {
    self.to_walker().read(buffer)
  }
  /// Seeks the chain to a given `Timecode`.
  pub fn seek(&self, pos: Timecode) -> Result<(), Error> {
    self.to_walker().seek(pos)
  }
  /// Returns the duration of the chain.
  pub fn duration(&self) -> Timecode {
    self.to_walker().duration()
  }
  /// Returns the `AudioInfo` of the final audio produced by the chain.
  pub fn info(&self) -> AudioInfo {
    self.info
  }

  fn to_walker(&self) -> AudioChainWalker {
    AudioChainWalker {
      chain: self.inner.clone(),
      filter_pos: if self.inner.filters.is_empty() {
        0
      } else {
        self.inner.filters.len()
      },
    }
  }
}

/// The `AudioChainWalker` allows filters to access the previous entry in the
/// `AudioChain`, whether that be a source or filter.
pub struct AudioChainWalker {
  chain: Rc<AudioChainInner>,
  filter_pos: usize,
}

impl AudioChainWalker {
  /// Writes samples to `buffer` from the previous entry in the chain.
  pub fn read(&mut self, buffer: &mut dyn PlanarAudioBuffer) -> Result<usize, Error> {
    if self.filter_pos == 0 {
      return self.chain.source.borrow_mut().read(buffer);
    }

    self.chain.filters[self.filter_pos - 1].borrow_mut().read(
      buffer,
      AudioChainWalker {
        chain: self.chain.clone(),
        filter_pos: self.filter_pos - 1,
      },
    )
  }
  /// Seeks the previous entry in the chain to `pos`.
  pub fn seek(&mut self, pos: Timecode) -> Result<(), Error> {
    if self.filter_pos == 0 {
      return self.chain.source.borrow_mut().seek(pos);
    }

    self.chain.filters[self.filter_pos - 1].borrow_mut().seek(
      pos,
      AudioChainWalker {
        chain: self.chain.clone(),
        filter_pos: self.filter_pos - 1,
      },
    )
  }
  /// Obtains the duration of the previous entry in the chain.
  pub fn duration(&self) -> Timecode {
    if self.filter_pos == 0 {
      return self.chain.source.borrow().duration();
    }

    self.chain.filters[self.filter_pos - 1]
      .borrow()
      .duration(AudioChainWalker {
        chain: self.chain.clone(),
        filter_pos: self.filter_pos - 1,
      })
  }
}

pub struct AudioChainBuilder {
  source: Box<dyn AudioSource>,
  filters: Vec<Box<dyn AudioFilter>>,
  info: AudioInfo,
}

impl AudioChainBuilder {
  pub fn new(source: impl AudioSource + 'static) -> AudioChainBuilder {
    let info = source.info();
    AudioChainBuilder {
      source: Box::new(source),
      filters: Vec::new(),
      info,
    }
  }

  pub const fn info(&self) -> AudioInfo {
    self.info
  }

  pub fn add_filter(&mut self, filter: impl AudioFilter + 'static) {
    self.info = filter.info();
    self.filters.push(Box::new(filter));
  }

  pub fn commit(self) -> AudioChain {
    AudioChain {
      inner: Rc::new(AudioChainInner {
        source: RefCell::new(self.source),
        filters: self.filters.into_iter().map(RefCell::new).collect(),
      }),
      info: self.info,
    }
  }
}
