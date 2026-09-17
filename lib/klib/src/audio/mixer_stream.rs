use std::collections::HashSet;

use crate::{
  error::Error,
  objects::{
    audio::AudioFileSource,
    event::EventValue,
    track::{Track, TrackValue},
  },
  timecode::Timecode,
};
use spectrasonic::{
  buffer::PlanarVecBuffer,
  filters::{WithChannelRemapperFilter, WithResamplerFilter},
  AudioChain, PlanarAudioBuffer,
};
use uuid::Uuid;

struct AudioMixerEventStream {
  #[allow(dead_code)]
  track_id: Uuid,
  event_id: Uuid,
  start_timecode: Timecode,
  end_timecode: Timecode,
  offset: Timecode,
  volume: f32,
  read_stream: AudioChain,
  read_buffer: PlanarVecBuffer,
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
  buffer: PlanarVecBuffer,
  // Duration in number of frames at `sample_rate`
  duration: usize,
  // Position in number of frames at `sample_rate`
  position: usize,
  block_size: usize,
}

impl AudioMixerStream {
  pub fn new(
    channels: usize,
    sample_rate: usize,
    block_size: usize,
  ) -> Result<Self, crate::error::Error> {
    Ok(Self {
      event_streams: Default::default(),
      time_factor: 1.0,
      channels,
      sample_rate,
      duration: 0,
      position: 0,
      buffer: PlanarVecBuffer::new(channels, block_size),
      block_size,
    })
  }

  pub fn sample_rate(&self) -> usize {
    self.sample_rate
  }

  pub fn channels(&self) -> usize {
    self.channels
  }

  pub fn duration_timecode(&self) -> Timecode {
    Timecode::from_seconds_f64(self.duration as f64 / self.sample_rate as f64)
  }

  pub fn position_timecode(&self) -> Timecode {
    Timecode::from_seconds_f64(self.position as f64 / self.sample_rate as f64)
  }

  pub fn seek(&mut self, new_timecode: Timecode) -> Result<(), Error> {
    let new_position =
      ((new_timecode.to_seconds_f64() * self.sample_rate as f64) as usize).min(self.duration);
    self.position = new_position;
    // Seek immediately in all of the events we can to start buffering them.
    for es in &mut self.event_streams {
      if new_timecode < es.start_timecode {
        es.read_stream.seek(Timecode(0).into())?;
        continue;
      } else if new_timecode >= es.end_timecode {
        es.read_stream.seek(es.read_stream.duration())?;
        continue;
      }

      let new_pos = (new_timecode - es.start_timecode + es.offset).into();
      es.read_stream.seek(new_pos)?;
    }

    Ok(())
  }

  /// Resets the position to zero and clears all loaded streams.
  pub fn reset(&mut self) {
    self.position = 0;
    self.event_streams.clear();
  }

  pub fn update_from_tracks(&mut self, tracks: &[Track]) -> Result<(), crate::error::Error> {
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
          let source = match &file.source {
            AudioFileSource::Path(path_buf) => spectrasonic::sources::source_for_file(path_buf)?,
            // TODO: handle managed files
            AudioFileSource::Managed => todo!(),
          };

          log::info!("loaded file {:?}", file.source);

          let mut builder = source.builder();
          if builder.info().num_channels > 1 && self.channels == 1 {
            builder = builder.with_channel_remapper(1, &[0])?;
          } else if builder.info().num_channels == 1 && self.channels == 2 {
            builder = builder.with_channel_remapper(2, &[0, 0])?;
          } else if builder.info().num_channels > 2 && self.channels == 2 {
            builder = builder.with_channel_remapper(2, &[0, 1])?;
          } else if builder.info().num_channels != self.channels {
            return Err(Error::Audio(format!(
              "Loaded file with {} channels but outputting {} - don't know how to remap",
              builder.info().num_channels,
              self.channels
            )));
          }

          if builder.info().sample_rate != self.sample_rate {
            builder = builder.with_resampler(self.sample_rate)?;
          }

          let chain = builder.commit();
          let buffer = PlanarVecBuffer::new(chain.info().num_channels, self.block_size);

          self.event_streams.push(AudioMixerEventStream {
            track_id: track.id,
            event_id: ev.id,
            volume: track_volume,
            offset: *offset,
            read_stream: chain,
            read_buffer: buffer,
            start_timecode: ev.start_timecode,
            end_timecode: ev.end_timecode,
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
  }

  /// Reads up to `self.block_size` frames into the planar `buffer`, returning
  /// the number of frames written.
  pub fn process_planar(&mut self, buffer: &mut [f32]) -> Result<usize, Error> {
    let num_read = self.process_impl()?;

    for ch in 0..self.channels {
      let channel = self.buffer.channel(ch);
      buffer[(ch * self.block_size)..((ch + 1) * self.block_size)].copy_from_slice(channel);
    }

    Ok(num_read)
  }

  /// Reads up to `self.block_size` frames into the interleaved `buffer`,
  /// returning the number of frames written.
  ///
  /// The internal representation of `AudioMixerStream` is planar, so this is
  /// more than a simple copy. If you can work with it, use `process_planar`.
  pub fn process_interleaved(&mut self, buffer: &mut [f32]) -> Result<usize, Error> {
    let num_read = self.process_impl()?;
    for ch in 0..self.channels {
      let channel = self.buffer.channel(ch);
      for i in 0..self.buffer.num_frames() {
        buffer[i * self.channels + ch] = channel[i];
      }
    }

    Ok(num_read)
  }

  fn process_impl(&mut self) -> Result<usize, Error> {
    let timecode =
      Timecode::from_seconds_f64(self.position as f64 / self.time_factor / self.sample_rate as f64);
    self.buffer.fill(0.0_f32);

    if self.position >= self.duration {
      return Ok(0);
    }

    for es in &mut self.event_streams {
      if es.start_timecode > timecode || es.end_timecode <= timecode {
        continue;
      }

      let num_read = es.read_stream.read(&mut es.read_buffer)?;
      for ch in 0..self.channels {
        let from_channel = es.read_buffer.channel_mut(ch);
        let to_channel = self.buffer.channel_mut(ch);
        for i in 0..num_read {
          to_channel[i] += from_channel[i] * es.volume;
        }
      }
    }

    self.position += self.block_size;

    Ok(self.block_size)
  }
}
