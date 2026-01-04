use std::collections::{HashMap, HashSet};

use audioadapter_buffers::number_to_float::SequentialNumbers;
use creek::{OpenError, ReadDiskStream, SymphoniaDecoder};
use klib::{
  objects::{
    audio::AudioFileSource,
    event::EventValue,
    track::{Track, TrackValue},
  },
  timecode::Timecode,
};
use rubato::Resampler;
use uuid::Uuid;

use crate::util::error::UiError;

pub const BLOCK_SIZE: usize = 1024;

struct AudioMixerEventStream {
  track_id: Uuid,
  event_id: Uuid,
  start_timecode: Timecode,
  end_timecode: Timecode,
  offset: Timecode,
  volume: f32,
  read_stream: ReadDiskStream<SymphoniaDecoder>,
  channels: usize,
  sample_rate: usize,
  resampler: Option<rubato::Fft<f32>>,
  read_block_size: usize,
  resample_buffer: Vec<f32>,
  read_buffer: Vec<Vec<f32>>,
}

impl AudioMixerEventStream {
  /// Computes the location of `position` within this event in frames.
  fn position_to_frame(&self, pos_sample_rate: usize, position: usize) -> usize {
    (position as f64 * (self.sample_rate as f64 / pos_sample_rate as f64)
      - (self.start_timecode.to_seconds_f64() * self.sample_rate as f64
        + self.offset.to_seconds_f64() * self.sample_rate as f64)) as usize
  }
}

/// The `AudioMixerStream` performs the raw audio processing necessary for
/// playback. It:
///
/// - Loads audio files from audio tracks.
/// - Resamples and mixes audio.
/// - Performs time stretching as necessary.
pub struct AudioMixerStream {
  event_streams: Vec<AudioMixerEventStream>,
  time_factor: f64,
  channels: usize,
  sample_rate: usize,

  // time_stretch_stream: bungee_rs::Stream,
  // Buffer of samples after timestretching.
  stretched_buffer: Vec<Vec<f32>>,
  /// `channels` numbers of `BLOCK_SIZE` buffers.
  planar_buffers: Vec<Vec<f32>>,

  // Duration in number of frames at `sample_rate`
  duration: usize,
  // Position in number of frames at `sample_rate`
  position: usize,
}

impl AudioMixerStream {
  pub fn new(channels: usize, sample_rate: usize) -> Result<Self, UiError> {
    let mut planar_buffers = Vec::new();
    for _ in 0..channels {
      let mut buffer = Vec::with_capacity(BLOCK_SIZE);
      buffer.resize(BLOCK_SIZE, 0.0f32);
      planar_buffers.push(buffer);
    }

    Ok(Self {
      event_streams: Default::default(),
      time_factor: 1.0,
      channels,
      sample_rate: 44100,
      duration: 0,
      position: 0,
      planar_buffers,
      stretched_buffer: Default::default(),
      // time_stretch_stream: bungee_rs::Stream::new(sample_rate, channels, BLOCK_SIZE)
      //   .map_err(|e| UiError::Audio(e.to_string()))?,
    })
  }

  pub fn sample_rate(&self) -> usize {
    self.sample_rate
  }

  pub fn channels(&self) -> usize {
    self.channels
  }

  pub fn position_timecode(&self) -> Timecode {
    Timecode::from_seconds_f64(self.position as f64 / self.sample_rate as f64)
  }

  pub fn seek(&mut self, new_timecode: Timecode) {
    let new_position = (new_timecode.to_seconds_f64() * self.sample_rate as f64) as usize;
    self.position = new_position;
    // Seek immediately in all of the events we can to start buffering them.
    for es in &mut self.event_streams {
      if new_timecode < es.start_timecode || new_timecode >= es.end_timecode {
        continue;
      }

      let _ = es.read_stream.seek(
        es.position_to_frame(self.sample_rate, new_position),
        creek::SeekMode::Auto,
      );
    }
  }

  /// Resets the position to zero and clears all loaded streams.
  pub fn reset(&mut self) {
    self.position = 0;
    self.event_streams.clear();
  }

  pub fn update_from_tracks(&mut self, tracks: &[Track]) -> Result<(), UiError> {
    let current_event_ids: HashSet<Uuid> =
      self.event_streams.iter().map(|es| es.event_id).collect();
    let mut new_event_ids: HashSet<Uuid> = Default::default();

    for track in tracks {
      let Some(TrackValue::Audio(track_audio)) = &track.track_value else {
        continue;
      };

      let track_volume = if track_audio.muted {
        0.0
      } else {
        track_audio.volume.powf(2.0)
      };

      for ev in track.events.iter() {
        let Some(EventValue::AudioClip { offset, file }) = &ev.value else {
          continue;
        };

        new_event_ids.insert(ev.id);

        if current_event_ids.contains(&ev.id) {
          // We already know about this event, just update its info.
          let Some(existing_event_stream) = self
            .event_streams
            .iter_mut()
            .find(|es| es.event_id == ev.id)
          else {
            continue;
          };

          existing_event_stream.volume = track_volume;
          existing_event_stream.offset = *offset;
          existing_event_stream.start_timecode = ev.start_timecode;
          existing_event_stream.end_timecode = ev.end_timecode;
        } else {
          // We don't know about this event yet, we need to load it.
          let mut stream: ReadDiskStream<SymphoniaDecoder> = match &file.source {
            AudioFileSource::Path(path_buf) => ReadDiskStream::new(path_buf, 0, Default::default())
              .map_err(|e: OpenError| UiError::Audio(e.to_string()))?,
            // TODO: handle managed files
            AudioFileSource::Managed => todo!(),
          };

          let sample_rate = stream.info().sample_rate.ok_or(UiError::Audio(
            "Tried to load audio file without sample rate".into(),
          ))? as usize;

          // Cache the start of the stream.
          stream
            .cache(0, (offset.to_seconds_f64() * sample_rate as f64) as usize)
            .map_err(|e| UiError::Audio(e.to_string()))?;

          stream.seek(0, creek::SeekMode::Auto).unwrap();

          stream.block_until_ready().unwrap();

          // If there are more than two channels, we pretend there's only two.
          let num_channels = stream.info().num_channels.max(2) as usize;

          // If we need to resample, let's preallocate the space we'll need.
          let read_block_size = if sample_rate != self.sample_rate {
            (BLOCK_SIZE as f64 * sample_rate as f64 / self.sample_rate as f64) as usize
          } else {
            BLOCK_SIZE
          };

          let resample_buffer = if sample_rate != self.sample_rate {
            let mut buffer = Vec::with_capacity(read_block_size);
            buffer.resize(read_block_size, 0.0f32);
            buffer
          } else {
            vec![]
          };

          let mut read_buffer = Vec::with_capacity(num_channels);
          for _ in 0..num_channels {
            let mut buffer = Vec::with_capacity(BLOCK_SIZE);
            buffer.resize(BLOCK_SIZE, 0.0f32);
            read_buffer.push(buffer);
          }

          self.event_streams.push(AudioMixerEventStream {
            track_id: track.id,
            event_id: ev.id,
            volume: track_volume,
            offset: *offset,
            channels: num_channels,
            read_stream: stream,
            sample_rate,
            start_timecode: ev.start_timecode,
            end_timecode: ev.end_timecode,
            resampler: if sample_rate != self.sample_rate {
              Some(
                rubato::Fft::<f32>::new(
                  sample_rate,
                  self.sample_rate,
                  1024,
                  1,
                  1,
                  rubato::FixedSync::Both,
                )
                .map_err(|e| UiError::Audio(e.to_string()))?,
              )
            } else {
              None
            },
            read_block_size,
            resample_buffer,
            read_buffer,
          });
        }
      }
    }

    // Remove event streams no longer present in the project.
    self
      .event_streams
      .retain(|es| new_event_ids.contains(&es.event_id));

    self.update_duration();

    Ok(())
  }

  /// Updates the duration and internal buffers.
  fn update_duration(&mut self) {
    self.duration = self
      .event_streams
      .iter()
      .map(|es| es.end_timecode)
      .max()
      .map(|t| ((t.to_seconds_f64() / self.time_factor) * self.sample_rate as f64).ceil() as usize)
      .unwrap_or_default();

    let stretched_block_size = BLOCK_SIZE as f64 * self.time_factor;
    let needed_raw_buffer = stretched_block_size.ceil() as usize;
    self.planar_buffers.clear();
    for _ in 0..self.channels {
      let mut buffer = Vec::with_capacity(needed_raw_buffer);
      buffer.resize(needed_raw_buffer, 0.0f32);
      self.planar_buffers.push(buffer);
    }
  }

  /// Fills `buffer` with up to `BLOCK_SIZE * self.channels` of interleaved
  /// audio.
  pub fn process(&mut self, buffer: &mut [f32]) -> Result<usize, UiError> {
    assert!(buffer.len() == BLOCK_SIZE * self.channels);

    // Stream has ended.
    if self.position >= self.duration {
      return Ok(0);
    }

    // Don't add overhead of time stretching if not necessary.
    let frame_count = if self.time_factor == 1.0 {
      self.process_raw()?;
      interleave_buffers(&self.planar_buffers, BLOCK_SIZE, buffer);
      BLOCK_SIZE
    } else {
      /*self.process_raw()?;
      let output_frames = self.time_stretch_stream.process(
        Some(&self.planar_buffers),
        &mut self.stretched_buffer,
        BLOCK_SIZE,
        BLOCK_SIZE as f64 / self.time_factor,
        1.0,
      );
      interleave_buffers(&self.stretched_buffer, output_frames, buffer);
      output_frames
      */
      0
    };

    self.position += frame_count;
    Ok(frame_count * self.channels)
  }

  // Obtain samples before time stretching.
  fn process_raw(&mut self) -> Result<(), UiError> {
    // Obtain the actual position in the stream from the timestretched position.
    let position = (self.position as f64 / self.time_factor).ceil() as usize;
    let timecode =
      Timecode::from_seconds_f64(self.position as f64 / self.time_factor / self.sample_rate as f64);
    // Initialize to zero.
    for i in 0..self.channels {
      self.planar_buffers[i].fill(0.0);
    }

    for es in &mut self.event_streams {
      // Event is not relevant.
      if es.start_timecode > timecode || es.end_timecode <= timecode {
        continue;
      }

      // Number of frames we are into the audio file.
      let frame_pos = es.position_to_frame(self.sample_rate, position);

      /*es.read_stream
      .seek(frame_pos, creek::SeekMode::Auto)
      .map_err(|e| UiError::Audio(e.to_string()))?;*/

      let data = es
        .read_stream
        .read(es.read_block_size)
        .map_err(|e| UiError::Audio(e.to_string()))?;

      assert!(data.num_channels() >= es.channels);

      if let Some(resampler) = &mut es.resampler {
        for i in 0..es.channels {
          es.resample_buffer.copy_from_slice(data.read_channel(i));
          let (_, output_frames) = resampler
            .process_into_buffer(
              &SequentialNumbers::new(es.resample_buffer.as_slice(), 1, es.read_block_size)
                .unwrap(),
              &mut SequentialNumbers::new_mut(&mut es.read_buffer[i], 1, BLOCK_SIZE).unwrap(),
              None,
            )
            .map_err(|e| UiError::Audio(e.to_string()))?;
          assert!(output_frames == BLOCK_SIZE);
        }
      } else {
        for i in 0..es.channels {
          es.read_buffer[i].copy_from_slice(data.read_channel(i));
        }
      }

      let frames = data.num_frames().min(BLOCK_SIZE);
      if es.channels == 2 {
        for i in 0..es.channels {
          for j in 0..frames {
            self.planar_buffers[i][j] += es.read_buffer[i][j] * es.volume;
          }
        }
      } else if es.channels == 1 && self.channels == 2 {
        for i in 0..self.channels {
          for j in 0..frames {
            self.planar_buffers[i][j] += es.read_buffer[0][j] * es.volume;
          }
        }
      } else if self.channels == 1 && es.channels == 2 {
        for i in 0..es.channels {
          for j in 0..frames {
            self.planar_buffers[0][j] += es.read_buffer[i][j] * es.volume;
          }
        }
      } else {
        log::warn!(
          "unknown channel combination, mixer {} event {}",
          self.channels,
          es.channels
        );
      }
    }

    Ok(())
  }
}

fn interleave_buffers(input: &[Vec<f32>], num_frames: usize, out_buffer: &mut [f32]) {
  let frame_len = input.len();
  for i in 0..frame_len {
    for j in 0..num_frames {
      out_buffer[j * frame_len + i] = input[i][j];
    }
  }
}
