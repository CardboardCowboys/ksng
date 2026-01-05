use std::sync::{Arc, Mutex, RwLock};

use circular_buffer::CircularBuffer;
use cpal::{
  SampleFormat,
  traits::{DeviceTrait, HostTrait, StreamTrait},
};
use klib::timecode::Timecode;

use crate::{
  audio::{
    config::AudioConfig,
    mixer_stream::{self, AudioMixerStream},
  },
  project::Project,
  util::{
    error::UiError,
    logger::{LogType, Logger},
  },
};

const BUFFER_SIZE: usize = 48000;

struct SharedOutputContext {
  mixer_stream: Mutex<AudioMixerStream>,
  buffer: RwLock<CircularBuffer<BUFFER_SIZE, f32>>,
  logger: Logger,
}

pub struct AudioMixer {
  output_stream: Box<dyn StreamTrait>,
  shared_context: Arc<SharedOutputContext>,
}

impl AudioMixer {
  pub fn new(config: &AudioConfig, logger: Logger) -> Result<Self, UiError> {
    let (stream, context) = Self::create_output_stream(config, logger)?;
    Ok(AudioMixer {
      output_stream: stream,
      shared_context: context,
    })
  }

  pub fn play(&self) {
    self.output_stream.play();
  }

  pub fn pause(&self) {
    // TODO: handle play state in stream or cpal
    self.output_stream.pause();
  }

  pub fn position(&self) -> Timecode {
    let mixer = self.shared_context.mixer_stream.lock().unwrap();
    let timecode = mixer.position_timecode();
    let sample_rate = mixer.sample_rate();
    let channels = mixer.channels();
    drop(mixer);

    Timecode::from_seconds_f64(
      timecode.to_seconds_f64()
        - self.shared_context.buffer.read().unwrap().len() as f64 / (sample_rate * channels) as f64,
    )
  }

  pub fn duration(&self) -> Timecode {
    self
      .shared_context
      .mixer_stream
      .lock()
      .unwrap()
      .duration_timecode()
  }

  pub fn seek(&self, time: Timecode) {
    self.shared_context.mixer_stream.lock().unwrap().seek(time);
    self.shared_context.buffer.write().unwrap().clear();
  }

  pub fn reset(&mut self) {
    self.shared_context.mixer_stream.lock().unwrap().reset()
  }

  pub fn update_streams(&mut self, project: &Project) -> Result<(), UiError> {
    self
      .shared_context
      .mixer_stream
      .lock()
      .unwrap()
      .update_from_tracks(&project.file.tracks)
  }

  pub fn update_audio_device(&mut self, config: &AudioConfig) -> Result<(), UiError> {
    let (stream, context) = Self::create_output_stream(config, self.shared_context.logger.clone())?;
    self.output_stream = stream;
    self.shared_context = context;
    Ok(())
  }

  fn create_output_stream(
    config: &AudioConfig,
    logger: Logger,
  ) -> Result<(Box<dyn StreamTrait>, Arc<SharedOutputContext>), UiError> {
    let device = config.to_device().unwrap_or_else(|| {
      log::warn!("Failed to find audio device {:?}, using default", config);
      cpal::default_host().default_output_device().unwrap()
    });

    let output_config = device
      .supported_output_configs()
      .ok()
      .and_then(|d| d.filter(|d| d.sample_format() == SampleFormat::F32).next())
      .ok_or(UiError::Audio(
        "Failed to get supported output configs for audio device.".into(),
      ))?;
    let output_config = output_config
      .try_with_sample_rate(44100)
      .unwrap_or(output_config.with_max_sample_rate());
    let context = Arc::new(SharedOutputContext {
      mixer_stream: Mutex::new(AudioMixerStream::new(
        output_config.channels() as usize,
        output_config.sample_rate() as usize,
      )?),
      logger,
      buffer: RwLock::new(CircularBuffer::new()),
    });

    log::info!(
      "creating audio device {} with {} channels, {} sample rate, {:?} buffer size",
      device.description().unwrap().driver().unwrap_or(""),
      output_config.channels(),
      output_config.sample_rate(),
      output_config.buffer_size()
    );

    let read_buffer_len = mixer_stream::BLOCK_SIZE * output_config.channels() as usize;
    let mut read_buffer = Vec::with_capacity(read_buffer_len);
    read_buffer.resize(read_buffer_len, 0.0f32);

    let ret_context = context.clone();
    let err_context = context.clone();
    let stream = device
      .build_output_stream::<f32, _, _>(
        &output_config.config(),
        move |buf, info| {
          let need_samples = buf.len();
          let mut mixer_stream = context.mixer_stream.lock().unwrap();
          let mut buffer = context.buffer.write().unwrap();
          while buffer.len() < need_samples {
            let Some(frames_written) = context.logger.wrap(mixer_stream.process(&mut read_buffer))
            else {
              return;
            };

            if frames_written == 0 {
              return;
            }

            buffer.extend_from_slice(&read_buffer[0..frames_written]);
          }

          for i in 0..buf.len() {
            buf[i] = buffer[i];
          }

          buffer.drain(0..buf.len());
        },
        move |err| {
          err_context
            .logger
            .log(LogType::Warning, format!("CPAL stream error: {:?}", err));
        },
        None,
      )
      .map_err(|e| UiError::Audio(format!("Failed to build CPAL output stream: {:?}", e)))?;
    Ok((Box::new(stream), ret_context))
  }
}
