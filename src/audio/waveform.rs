use std::{
  collections::HashMap,
  io::Write,
  path::{Path, PathBuf},
  sync::{Arc, RwLock},
  thread,
};

use egui::Context;
use klib::objects::{
  audio::AudioFileSource,
  event::{Event, EventValue},
};
use log::error;
use symphonia::core::{
  audio::SampleBuffer, codecs::DecoderOptions, formats::FormatOptions, io::MediaSourceStream,
  meta::MetadataOptions, probe::Hint,
};
use tiny_skia::{Color, Paint, Pixmap, Rect, Transform};
use uuid::Uuid;

use crate::{components::timeline, fs::Cache, util::error::UiError, util::logger::Logger};

const BLOCK_SIZE: usize = 2048;
const DYNAMIC_RANGE: f32 = 48.0;

pub struct WaveformGenerator {
  height: usize,
}

impl WaveformGenerator {
  pub fn create_waveform(&self, path: &Path) -> Result<Vec<u8>, UiError> {
    let file = std::fs::File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let hint = Hint::new();

    let format_opts: FormatOptions = Default::default();
    let metadata_opts: MetadataOptions = Default::default();
    let decoder_opts: DecoderOptions = Default::default();

    let probed =
      symphonia::default::get_probe().format(&hint, mss, &format_opts, &metadata_opts)?;

    let mut format = probed.format;
    let track = format.default_track().unwrap();
    let mut decoder = symphonia::default::get_codecs().make(&track.codec_params, &decoder_opts)?;

    let track_id = track.id;

    let mut sample_buf = None;

    let mut pending_sample_count = 0;
    let mut pending_peak_sum = 0.0;

    let mut peaks: Vec<f32> = Vec::new();

    loop {
      let packet = match format.next_packet() {
        Ok(packet) => packet,
        Err(symphonia::core::errors::Error::IoError(error))
          if error.kind() == std::io::ErrorKind::UnexpectedEof =>
        {
          break;
        }
        Err(err) => return Err(err.into()),
      };

      if packet.track_id() != track_id {
        continue;
      }

      let audio_buf = decoder.decode(&packet)?;
      if sample_buf.is_none() {
        let spec = *audio_buf.spec();
        let duration = audio_buf.capacity() as u64;
        sample_buf = Some(SampleBuffer::<f32>::new(duration, spec));
      }

      if let Some(buf) = &mut sample_buf {
        buf.copy_interleaved_ref(audio_buf);
        let samples = buf.samples();

        for sample in samples {
          pending_sample_count += 1;
          pending_peak_sum += *sample * *sample;

          if pending_sample_count >= BLOCK_SIZE {
            peaks.push(Self::create_peak(pending_peak_sum, pending_sample_count));
            pending_peak_sum = 0.0;
            pending_sample_count = 0;
          }
        }
      }
    }

    if pending_sample_count > 0 {
      peaks.push(Self::create_peak(pending_peak_sum, pending_sample_count));
    }

    self.render_peaks(peaks)
  }

  fn create_peak(sum: f32, count: usize) -> f32 {
    f32::sqrt(sum / count as f32)
  }

  fn render_peaks(&self, peaks: Vec<f32>) -> Result<Vec<u8>, UiError> {
    let width = peaks.len();
    let mut pixmap = Pixmap::new(width as u32, self.height as u32).ok_or(UiError::Audio(
      "Can't get pixmap to draw waveform".to_string(),
    ))?;
    pixmap.fill(Color::TRANSPARENT);
    let mut paint = Paint::default();
    paint.set_color_rgba8(0, 0, 0, 255);

    let center_y = (self.height as f32 / 2.0).floor();
    for (i, peak) in peaks.iter().enumerate() {
      let peak = (*peak).clamp(0.05, 1.0);
      let start_y = center_y - (peak * center_y);
      let end_y = center_y + (peak * center_y);
      pixmap.fill_rect(
        Rect::from_ltrb(i as f32, start_y, (i + 1) as f32, end_y).unwrap(),
        &paint,
        Transform::identity(),
        None,
      );
    }

    pixmap
      .encode_png()
      .map_err(|e| UiError::Audio(e.to_string()))
  }
}

enum WaveformState {
  Loading,
  Loaded(Arc<String>),
  Failed,
}

type WaveformsMap = Arc<RwLock<HashMap<Uuid, WaveformState>>>;

pub struct AudioWaveformProvider {
  waveforms: WaveformsMap,
  logger: Logger,
}

impl AudioWaveformProvider {
  pub fn new(logger: Logger) -> AudioWaveformProvider {
    AudioWaveformProvider {
      waveforms: Default::default(),
      logger,
    }
  }

  pub fn clear(&mut self, ctx: &Context) {
    for v in self.waveforms.read().unwrap().values() {
      if let WaveformState::Loaded(path) = &v {
        ctx.forget_image(path);
      }
    }

    self.waveforms.write().unwrap().clear();
  }

  pub fn get_image(&self, event: &Event) -> Option<Arc<String>> {
    if let Some(state) = self.waveforms.read().ok()?.get(&event.id) {
      return match state {
        WaveformState::Loading | WaveformState::Failed => None,
        WaveformState::Loaded(image) => Some(image.clone()),
      };
    }

    let cache_path = Cache::get_file_path(event.id, "png").ok()?;
    if std::fs::exists(&cache_path).unwrap_or(false) {
      let image: Arc<String> = Arc::new(Self::to_file_uri(&cache_path).ok()?);
      self
        .waveforms
        .write()
        .ok()?
        .insert(event.id, WaveformState::Loaded(image.clone()));
      return Some(image);
    }

    let audio_path = event.value.as_ref().and_then(|v| match v {
      EventValue::AudioClip { file, .. } => match &file.source {
        AudioFileSource::Path(path_buf) => Some(path_buf.clone()),
        AudioFileSource::Managed => None,
      },
      _ => None,
    });

    if let Some(path) = audio_path {
      self
        .waveforms
        .write()
        .ok()?
        .insert(event.id, WaveformState::Loading);
      self.load_waveform(event.id, path);
    } else {
      error!("Failed to load audio file ID {}, no path found", event.id);
      self
        .waveforms
        .write()
        .ok()?
        .insert(event.id, WaveformState::Failed);
    }
    None
  }

  fn load_waveform(&self, id: Uuid, path: PathBuf) {
    let logger = self.logger.clone();
    let waveforms = self.waveforms.clone();
    thread::spawn(move || {
      let path = logger.wrap(Self::load_waveform_impl(id, path));
      let mut waveforms = waveforms.write().ok();
      if let Some(waveforms) = waveforms.as_mut() {
        if let Some(path) = path {
          waveforms.insert(id, WaveformState::Loaded(Arc::new(path)));
        } else {
          waveforms.insert(id, WaveformState::Failed);
        }
      }
    });
  }

  fn load_waveform_impl(id: Uuid, path: PathBuf) -> Result<String, UiError> {
    let generator = WaveformGenerator {
      height: timeline::TRACK_HEIGHT as usize,
    };
    let buffer = generator.create_waveform(path.as_ref())?;
    let cache_file = Cache::get_file_path(id, "png")?;
    let mut file = std::fs::File::create(&cache_file)?;
    file.write_all(&buffer)?;
    drop(file);
    Self::to_file_uri(&cache_file)
  }

  fn to_file_uri(path: &Path) -> Result<String, UiError> {
    let mut s = String::with_capacity(path.to_str().map(|l| l.len()).unwrap_or(0));
    s.push_str("file://");
    for comp in path.components() {
      s.push('/');
      s.push_str(
        comp
          .as_os_str()
          .to_str()
          .ok_or(UiError::Io("Can't create URI from file path".to_string()))?,
      );
    }
    Ok(s)
  }
}
