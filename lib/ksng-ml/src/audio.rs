use std::{
  io::Write,
  path::{Path, PathBuf},
};

use creek::{ReadDiskStream, SymphoniaDecoder};
use ndarray::s;
use rubato::{Resampler, audioadapter::Adapter};
use zerocopy::IntoBytes;

const fn chunk_size(chunk_s: f64, target_sample_rate: usize) -> usize {
  (target_sample_rate as f64 * chunk_s) as usize
}

const fn overlap_size(chunk_size: usize) -> usize {
  chunk_size / 4
}

pub struct AudioChunkProvider {
  target_sample_rate: usize,
  from_sample_rate: usize,
  chunk_frames: usize,
  overlap: usize,
  channels: usize,
  total_frames: usize,
  file_buffer: Vec<Vec<f32>>,
  current_chunk: usize,
}

impl AudioChunkProvider {
  pub fn new(
    file_path: &Path,
    target_sample_rate: usize,
    chunk_s: f64,
  ) -> Result<AudioChunkProvider, anyhow::Error> {
    let mut decoder = ReadDiskStream::<SymphoniaDecoder>::new(file_path, 0, Default::default())?;
    decoder.cache(0, 0)?;
    decoder.seek(0, creek::SeekMode::Auto).unwrap();
    decoder.block_until_ready()?;

    let num_channels = decoder.info().num_channels as usize;
    let from_sample_rate = decoder
      .info()
      .sample_rate
      .ok_or(anyhow::Error::msg("File has no sample rate!"))? as usize;

    let chunk_frames = chunk_size(chunk_s, target_sample_rate);

    // For now, read the entire file into the buffer... I'm not writing another
    // audio streaming implementation.
    let file_frames = decoder.info().num_frames;
    let mut buffer = Vec::with_capacity(num_channels);
    for ch in 0..num_channels {
      let mut ch_buffer = Vec::with_capacity(file_frames);
      ch_buffer.resize(file_frames, 0.0_f32);
      buffer.push(ch_buffer);
    }

    let mut frames_written = 0;
    // Read entire file into buffer
    loop {
      decoder.block_until_ready().unwrap();
      let data = decoder.read(decoder.block_size())?;
      for ch in 0..data.num_channels() {
        buffer[ch][frames_written..(frames_written + data.num_frames())]
          .copy_from_slice(data.read_channel(ch));
      }

      frames_written += data.num_frames();
      if data.num_frames() < decoder.block_size() {
        break;
      }
    }

    if from_sample_rate != target_sample_rate {
      let mut resampler = rubato::Fft::<f32>::new(
        from_sample_rate,
        target_sample_rate,
        1024,
        2,
        rubato::FixedSync::Both,
      )?;

      let slice = audioadapter_buffers::direct::SequentialSliceOfVecs::new(
        &buffer,
        num_channels,
        frames_written,
      )?;
      let result = resampler.process_all(&slice, frames_written, None)?;

      buffer.clear();
      for ch in 0..num_channels {
        let mut ch_buffer = Vec::with_capacity(result.frames());
        ch_buffer.resize(result.frames(), 0.0_f32);
        result.copy_from_channel_to_slice(ch, 0, &mut ch_buffer);
        buffer.push(ch_buffer);
      }

      frames_written = result.frames();
    }

    Ok(AudioChunkProvider {
      target_sample_rate,
      from_sample_rate,
      channels: num_channels,
      chunk_frames,
      overlap: overlap_size(chunk_frames),
      file_buffer: buffer,
      total_frames: frames_written,
      current_chunk: 0,
    })
  }

  pub fn next_chunk(&mut self, buffer: &mut ndarray::Array2<f32>) -> Result<usize, anyhow::Error> {
    let (d1, d2) = buffer.dim();
    assert!(d1 == self.channels && d2 == self.chunk_frames);

    let stride = self.chunk_frames - self.overlap;
    let start = self.current_chunk * stride;
    let end = (start + self.chunk_frames).min(self.total_frames);
    let num_frames = end.saturating_sub(start);

    if start >= end {
      return Ok(0);
    }

    for ch in 0..self.channels {
      let mut row = buffer.row_mut(ch);
      let dest = row.as_slice_mut().unwrap();
      dest[0..num_frames].copy_from_slice(&self.file_buffer[ch][start..end]);
      if num_frames < self.chunk_frames {
        dest[num_frames..self.chunk_frames].fill(0.0_f32);
      }
    }

    self.current_chunk += 1;

    Ok(num_frames)
  }

  pub const fn chunk_size(&self) -> usize {
    self.chunk_frames
  }

  pub const fn total_samples(&self) -> usize {
    self.total_frames * self.channels
  }

  pub const fn total_frames(&self) -> usize {
    self.total_frames
  }

  pub const fn overlap(&self) -> usize {
    self.overlap
  }
}

pub struct AudioChunkWriter {
  buffers: Vec<Vec<f32>>,
  output_paths: Vec<PathBuf>,
  total_frames: usize,
  num_channels: usize,
  chunk_size: usize,
  overlap: usize,
  current_chunk: usize,
  chunk_buf: Vec<f32>,
}

impl AudioChunkWriter {
  pub fn new(
    output_paths: &[PathBuf],
    total_frames: usize,
    num_channels: usize,
    chunk_size: usize,
  ) -> Result<AudioChunkWriter, anyhow::Error> {
    let mut buffers = Vec::with_capacity(output_paths.len());
    for _ in output_paths {
      let mut buffer = Vec::with_capacity(total_frames * num_channels);
      buffer.resize(total_frames * num_channels, 0.0_f32);
      buffers.push(buffer);
    }

    let mut chunk_buf = Vec::with_capacity(chunk_size);
    chunk_buf.resize(chunk_size, 0.0_f32);

    Ok(AudioChunkWriter {
      buffers,
      output_paths: output_paths.iter().map(|p| p.to_path_buf()).collect(),
      total_frames,
      num_channels,
      chunk_size,
      overlap: overlap_size(chunk_size),
      current_chunk: 0,
      chunk_buf,
    })
  }

  pub fn write_chunk(&mut self, buffer: &ndarray::ArrayView4<f32>) -> Result<(), anyhow::Error> {
    let stride = self.chunk_size - self.overlap;
    let first_chunk = self.current_chunk == 0;
    let last_chunk = (self.current_chunk + 1) * stride >= self.total_frames;
    let fade_out_start = self.chunk_size - self.overlap;

    for i in 0..self.output_paths.len() {
      for ch in 0..self.num_channels {
        let slice = buffer.slice(s![0, i, ch, ..]);
        let samples = slice.as_slice().unwrap();
        for (j, sample) in samples.into_iter().enumerate() {
          let out = (self.current_chunk * stride + j) * self.num_channels + ch;
          if out >= (self.total_frames * self.num_channels) {
            break;
          }
          if !first_chunk && j < self.overlap {
            let n = j as f32 / self.overlap as f32;
            self.buffers[i][out] += sample * n;
          } else if !last_chunk && j >= fade_out_start {
            let n = 1.0 - ((j - fade_out_start) as f32 / self.overlap as f32);
            self.buffers[i][out] += sample * n;
          } else {
            self.buffers[i][out] += sample;
          }
        }
      }
    }

    self.current_chunk += 1;

    Ok(())
  }

  pub fn finalize(&self) -> Result<(), anyhow::Error> {
    for (i, buffer) in self.buffers.iter().enumerate() {
      let path = &self.output_paths[i];
      let mut stream = std::fs::File::create(path)?;
      stream.write_all(buffer.as_bytes())?;
    }

    Ok(())
  }
}
