use std::path::{Path, PathBuf};

use flacenc::{
  component::{BitRepr, Stream},
  config::Encoder,
  error::{Verified, Verify},
  source::{Fill, FrameBuf},
};

use crate::{
  Error, PlanarAudioBuffer, buffer::PlanarVecBuffer, chain::AudioInfo, encoders::AudioEncoder,
};

pub struct FlacAudioEncoder {
  output_path: PathBuf,
  info: AudioInfo,
  enc: Verified<Encoder>,
  stream: Stream,
  block: PlanarVecBuffer,
  conv_block: Vec<i32>,
  frame_buf: FrameBuf,
  waiting_frames: usize,
  frame_number: usize,
}

impl FlacAudioEncoder {
  pub fn new<P: AsRef<Path>>(input: P, info: AudioInfo) -> Result<FlacAudioEncoder, Error> {
    let conf = flacenc::config::Encoder::default().into_verified().unwrap();
    let block = PlanarVecBuffer::new(info.num_channels, conf.block_size);
    let stream = Stream::new(info.sample_rate, info.num_channels, 16)?;

    let mut conv_block = Vec::with_capacity(conf.block_size);
    conv_block.resize(conf.block_size * info.num_channels, 0);

    Ok(FlacAudioEncoder {
      output_path: input.as_ref().with_extension("flac"),
      info,
      stream,
      conv_block,
      frame_buf: FrameBuf::with_size(info.num_channels, conf.block_size)?,
      enc: conf,
      block,
      waiting_frames: 0,
      frame_number: 0,
    })
  }

  fn write_frame(&mut self, ofs: usize, len: usize) -> Result<(), Error> {
    for ch in 0..self.info.num_channels {
      let data = &self.block.channel(ch)[ofs..(ofs + len)];
      for (i, sample) in data.iter().enumerate() {
        let s = (sample.clamp(-1.0, f32::next_down(1.0)) * 32_768.0) as i32;
        self.conv_block[i * self.info.num_channels + ch] = s;
      }
      let written = data.len();
      let remaining = self.block.num_frames() - data.len();
      for i in 0..remaining {
        self.conv_block[(written + i) * self.info.num_channels + ch] = 0;
      }
    }

    self.frame_buf.fill_interleaved(&self.conv_block)?;

    let frame = flacenc::encode_fixed_size_frame(
      &self.enc,
      &self.frame_buf,
      self.frame_number,
      self.stream.stream_info(),
    )?;

    self.stream.add_frame(frame);
    self.frame_number += 1;
    self.waiting_frames = 0;

    Ok(())
  }
}

impl AudioEncoder for FlacAudioEncoder {
  fn write(&mut self, buffer: &dyn crate::PlanarAudioBuffer) -> Result<(), crate::Error> {
    let mut num_read = 0;
    loop {
      let num_expected = self.block.num_frames() - self.waiting_frames;
      let num = num_expected.min(buffer.num_frames() - num_read);
      self
        .block
        .copy_from_buffer(buffer, num_read, self.waiting_frames, num);
      self.waiting_frames = num;
      num_read += num;

      if num < num_expected {
        break;
      }

      self.write_frame(0, self.block.num_frames())?;
    }

    Ok(())
  }

  fn finalize(&mut self) -> Result<(), crate::Error> {
    if self.waiting_frames > 0 {
      self.write_frame(
        self.waiting_frames,
        self.block.num_frames() - self.waiting_frames,
      )?;
    }

    let mut sink = flacenc::bitsink::ByteSink::new();
    self
      .stream
      .write(&mut sink)
      .map_err(|e| Error::Flac(e.to_string()))?;
    std::fs::write(&self.output_path, sink.as_slice())?;
    Ok(())
  }
}
