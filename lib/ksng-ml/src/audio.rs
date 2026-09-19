use std::path::{Path, PathBuf};

use itertools::izip;
use ndarray::s;
use spectrasonic::{
  AudioChain, PlanarAudioBuffer,
  buffer::PlanarVecBuffer,
  chain::AudioInfo,
  encoders::ffmpeg::{FfmpegAudioEncoder, FfmpegAudioEncoderOptions},
  filters::{WithChannelRemapperFilter, WithResamplerFilter},
};

const fn chunk_size(chunk_s: f64, target_sample_rate: usize) -> usize {
  (target_sample_rate as f64 * chunk_s) as usize
}

const fn overlap_size(chunk_size: usize) -> usize {
  chunk_size / 4
}

pub struct AudioChunkProvider {
  target_sample_rate: usize,
  chunk_frames: usize,
  overlap: usize,
  channels: usize,
  total_frames: usize,
  current_chunk: usize,
  current_source_chunk: usize,
  source: AudioChain,
  chunk_buffer: PlanarVecBuffer,
}

impl AudioChunkProvider {
  pub fn new(
    file_path: &Path,
    target_sample_rate: usize,
    target_channels: usize,
    chunk_s: f64,
  ) -> Result<AudioChunkProvider, anyhow::Error> {
    let source = spectrasonic::sources::source_for_file(file_path)?;
    let mut builder = source.builder();

    if target_sample_rate != builder.info().sample_rate {
      builder = builder.with_resampler(target_sample_rate)?;
    }

    if target_channels != builder.info().num_channels {
      if builder.info().num_channels == 1 && target_channels == 2 {
        builder = builder.with_channel_remapper(2, &[0, 0])?;
      } else if builder.info().num_channels > 2 && target_channels == 2 {
        builder = builder.with_channel_remapper(2, &[0, 1])?;
      } else if builder.info().num_channels > 1 && target_channels == 1 {
        builder = builder.with_channel_remapper(1, &[0])?;
      } else {
        return Err(anyhow::Error::msg(format!(
          "Can't figure out how to remap {} channels to {target_channels} channels",
          builder.info().num_channels
        )));
      }
    }

    let chunk_frames = chunk_size(chunk_s, target_sample_rate);
    let chunk_buffer = PlanarVecBuffer::new(target_channels, chunk_frames);
    let source = builder.commit();
    let duration = source.duration();
    log::info!("duration: {duration:?} {}", duration.to_seconds_f64());

    Ok(AudioChunkProvider {
      target_sample_rate,
      channels: source.info().num_channels,
      chunk_frames,
      overlap: overlap_size(chunk_frames),
      chunk_buffer,
      total_frames: duration.to_frames_at_rate(target_sample_rate),
      current_source_chunk: 0,
      current_chunk: 0,
      source,
    })
  }

  pub fn next_chunk(&mut self, buffer: &mut ndarray::Array2<f32>) -> Result<usize, anyhow::Error> {
    let (d1, d2) = buffer.dim();
    assert!(d1 == self.channels && d2 == self.chunk_frames);

    let stride = self.chunk_frames - self.overlap;
    let start = self.current_chunk * stride;
    let end = (start + self.chunk_frames).min(self.total_frames);
    let num_frames = end.saturating_sub(start);
    let source_pos = self.current_source_chunk * self.chunk_frames;

    if start >= end {
      return Ok(0);
    }

    let mut frames_written = 0;
    let num_current = source_pos - start;
    if num_current > 0 {
      let num = num_current.min(num_frames);
      for ch in 0..self.channels {
        let mut row = buffer.row_mut(ch);
        let dest = row.as_slice_mut().unwrap();
        let n = self.chunk_buffer.num_frames();
        dest[0..num].copy_from_slice(&self.chunk_buffer.channel(ch)[(n - num)..n]);
      }
      frames_written = num;
    }

    if frames_written >= self.chunk_frames {
      self.current_chunk += 1;
      return Ok(frames_written);
    }

    let frames_read = self.source.read(&mut self.chunk_buffer)?;
    self.current_source_chunk += 1;
    let num = frames_read.min(self.chunk_frames - frames_written);
    for ch in 0..self.channels {
      let mut row = buffer.row_mut(ch);
      let dest = row.as_slice_mut().unwrap();
      let end = frames_written + num;
      dest[frames_written..end].copy_from_slice(&self.chunk_buffer.channel(ch)[0..num]);
      if end < self.chunk_frames {
        dest[end..self.chunk_frames].fill(0.0_f32);
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
  encoders: Vec<FfmpegAudioEncoder>,
  output_paths: Vec<PathBuf>,
  full_chunks: Vec<PlanarVecBuffer>,
  crossfade_chunks: Vec<PlanarVecBuffer>,
  chunk_size: usize,
  total_frames: usize,
  overlap: usize,
  current_chunk: usize,
}

impl AudioChunkWriter {
  pub fn new(
    output_paths: &[PathBuf],
    options: FfmpegAudioEncoderOptions,
    total_frames: usize,
    num_channels: usize,
    sample_rate: usize,
    chunk_size: usize,
  ) -> Result<AudioChunkWriter, anyhow::Error> {
    let mut encoders = Vec::with_capacity(output_paths.len());
    let mut chunks = Vec::with_capacity(output_paths.len());
    let mut crossfade_chunks = Vec::with_capacity(output_paths.len());
    let mut paths = Vec::with_capacity(output_paths.len());
    let overlap = overlap_size(chunk_size);
    for path in output_paths {
      encoders.push(FfmpegAudioEncoder::new(
        path,
        options.clone(),
        AudioInfo {
          num_channels,
          sample_rate,
        },
      )?);
      chunks.push(PlanarVecBuffer::new(num_channels, chunk_size - overlap * 2));
      crossfade_chunks.push(PlanarVecBuffer::new(num_channels, overlap));
      paths.push(options.codec.set_extension(path));
    }

    log::info!("duration: {}", total_frames as f64 / sample_rate as f64);

    Ok(AudioChunkWriter {
      output_paths: paths,
      encoders,
      full_chunks: chunks,
      crossfade_chunks,
      overlap: overlap_size(chunk_size),
      chunk_size,
      total_frames,
      current_chunk: 0,
    })
  }

  pub fn write_chunk(&mut self, buffer: &ndarray::ArrayView4<f32>) -> Result<(), anyhow::Error> {
    let stride = self.chunk_size - self.overlap;
    let first_chunk = self.current_chunk == 0;
    let last_chunk = (self.current_chunk + 1) * stride >= self.total_frames;

    // The output stream is basically:
    // - [crossfade][chunk][crossfade][chunk][crossfade][chunk]
    // So we need to:
    // - Mix [crossfade] samples into the crossfade chunk from last write
    // - Send [crossfade] to the encoder.
    // - Send [chunk] to the encoder.
    // - Write [crossfade] samples into the crossfade chunk for next write.
    // Two special cases:
    // - On the first chunk we send [crossfade] directly to the encoder and do
    //   not fade it.
    // - On the last chunk we send the last [crossfade] directly to the encoder
    //   and do not fade it.

    for (out_idx, (enc, chunk, crossfade)) in izip!(
      self.encoders.iter_mut(),
      self.full_chunks.iter_mut(),
      self.crossfade_chunks.iter_mut()
    )
    .enumerate()
    {
      for ch in 0..chunk.num_channels() {
        let slice = buffer.slice(s![0, out_idx, ch, ..]);
        let samples = slice.as_slice().unwrap();
        if first_chunk {
          crossfade
            .channel_mut(ch)
            .copy_from_slice(&samples[0..self.overlap]);
        } else {
          let out = crossfade.channel_mut(ch);
          for i in 0..self.overlap {
            out[i] += (i as f32 / self.overlap as f32) * samples[i];
          }
        }
      }

      enc.write(crossfade)?;

      for ch in 0..chunk.num_channels() {
        let slice = buffer.slice(s![0, out_idx, ch, ..]);
        let samples = &slice.as_slice().unwrap()[self.overlap..(self.overlap + chunk.num_frames())];
        chunk.channel_mut(ch).copy_from_slice(samples);
      }

      enc.write(chunk)?;

      for ch in 0..chunk.num_channels() {
        let slice = buffer.slice(s![0, out_idx, ch, ..]);
        let samples = &slice.as_slice().unwrap()[(self.overlap + chunk.num_frames())..slice.len()];
        if last_chunk {
          crossfade.channel_mut(ch).copy_from_slice(&samples);
        } else {
          let out = crossfade.channel_mut(ch);
          for i in 0..self.overlap {
            out[i] = (1.0 - (i as f32 / self.overlap as f32)) * samples[i];
          }
        }
      }
    }

    self.current_chunk += 1;

    Ok(())
  }

  pub fn finalize(&mut self) -> Result<Vec<PathBuf>, anyhow::Error> {
    for (enc, crossfade) in self.encoders.iter_mut().zip(self.crossfade_chunks.iter()) {
      enc.write(crossfade)?;
      enc.finalize()?;
    }

    Ok(self.output_paths.clone())
  }
}
