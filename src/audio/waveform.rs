use std::{
  collections::{HashMap, LinkedList},
  io::Read,
  path::{Path, PathBuf},
  sync::{Arc, RwLock},
  thread,
};

use egui::{Context, load::Bytes};
use klib::objects::{
  attachment::{AttachmentReader, AttachmentResolver, AttachmentSource},
  audio::AudioFileSource,
  event::{Event, EventValue},
  file::File,
};
use log::error;
use spectrasonic::{PlanarAudioBuffer, buffer::PlanarVecBuffer};
use tar::{Archive, Builder, Header};
use tiny_skia::{Color, Paint, Pixmap, Rect, Transform};
use uuid::Uuid;

use crate::{
  components::timeline,
  fs::{Cache, KsngAttachmentResolver},
  util::{error::UiError, logger::Logger},
};

const BASE_BLOCK_SIZE: usize = 8192;
pub const MAX_TEXTURE_SIZE: usize = 2048;

struct WaveformResult {
  levels: Vec<Vec<Vec<u8>>>,
  pixel_to_s: Vec<f32>,
  duration: f32,
}

pub struct WaveformCacheInfo {
  pub levels: usize,
  pub imgs_per_level: Vec<usize>,
  pub pixel_to_s: Vec<f32>,
  pub duration: f32,
}

impl WaveformResult {
  pub fn save_to_archive(&self, out_path: &Path) -> Result<(), UiError> {
    let writer = std::fs::File::create(out_path)?;
    let mut builder = Builder::new(writer);

    assert!(self.levels.len() <= 0xff);

    // first file has info on how many mip levels and how many images per level
    let mut header = Header::new_gnu();
    header.set_path(".shape")?;
    let mut data = Vec::new();
    data.push(self.levels.len() as u8);
    let dur = self.duration.to_le_bytes();
    data.push(dur[0]);
    data.push(dur[1]);
    data.push(dur[2]);
    data.push(dur[3]);
    for (i, level) in self.levels.iter().enumerate() {
      assert!(level.len() <= 0xffff);
      let len = (level.len() as u16).to_le_bytes();
      data.push(len[0]);
      data.push(len[1]);
      let pxs = self.pixel_to_s[i].to_le_bytes();
      data.push(pxs[0]);
      data.push(pxs[1]);
      data.push(pxs[2]);
      data.push(pxs[3]);
    }
    header.set_size(data.len() as u64);
    header.set_cksum();
    builder.append(&header, data.as_slice())?;

    for (l, level) in self.levels.iter().enumerate() {
      for (idx, img) in level.iter().enumerate() {
        let mut header = Header::new_gnu();
        header.set_path(format!("mip-l{l}-i{idx}.png"))?;
        header.set_size(img.len() as u64);
        header.set_cksum();
        builder.append(&header, img.as_slice())?;
      }
    }

    Ok(())
  }

  pub fn load_info_from_archive(path: &Path) -> Result<WaveformCacheInfo, UiError> {
    let reader = std::fs::File::open(path)?;
    let mut ar = Archive::new(reader);
    let Some(Ok(mut shape_entry)) = ar.entries()?.next() else {
      return Err(UiError::Audio(format!(
        "Failed to load info from archive for {path:?}"
      )));
    };

    let levels = shape_entry.read_le::<u8>()? as usize;
    let duration = shape_entry.read_le::<f32>()?;
    let mut levels_vec = Vec::with_capacity(levels);
    let mut pixel_to_s = Vec::with_capacity(levels);
    for _ in 0..levels {
      let images = shape_entry.read_le::<u16>()? as usize;
      levels_vec.push(images);
      let pxs = shape_entry.read_le::<f32>()?;
      pixel_to_s.push(pxs);
    }

    Ok(WaveformCacheInfo {
      levels,
      imgs_per_level: levels_vec,
      pixel_to_s,
      duration,
    })
  }

  pub fn load_img_from_archive(path: &Path, level: usize, idx: usize) -> Result<Vec<u8>, UiError> {
    let reader = std::fs::File::open(path)?;
    let mut ar = Archive::new(reader);
    let target_name = format!("mip-l{level}-i{idx}.png");

    for entry in ar.entries_with_seek()? {
      let mut entry = entry?;
      if let Some(name) = entry.path()?.file_name().and_then(|s| s.to_str())
        && name == target_name
      {
        let mut buf = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut buf)?;
        return Ok(buf);
      }
    }

    Err(UiError::Audio(format!(
      "Could not find mip level {level} idx {idx} at path {path:?}"
    )))
  }
}

/// Generates multiple mipmaps worth of waveforms for a file.
/// Ensures each produced image is no more than MAX_TEXTURE_SIZE pixels in
/// width.
pub struct WaveformGenerator {
  height: usize,
  mip_levels: usize,
}

impl WaveformGenerator {
  fn create_waveform(&self, path: &Path) -> Result<WaveformResult, UiError> {
    let mut source = spectrasonic::sources::source_for_file(path)?;
    let duration = source.duration().to_seconds();

    let mut n_total_blocks: Vec<usize> = Vec::with_capacity(self.mip_levels);
    let mut peaks: Vec<Vec<f32>> = Vec::with_capacity(self.mip_levels);
    let mut block_sizes = Vec::with_capacity(self.mip_levels);
    for i in 0..self.mip_levels {
      peaks.push(Vec::new());
      block_sizes.push(BASE_BLOCK_SIZE / 2_usize.pow((i + 1) as u32));
      n_total_blocks.push(0);
    }

    let mut block = PlanarVecBuffer::new(source.info().num_channels, BASE_BLOCK_SIZE);

    loop {
      let frames = source.read(&mut block)?;
      if frames == 0 {
        break;
      }

      for i in 0..self.mip_levels {
        let n_blocks = frames.div_ceil(block_sizes[i]);
        for b in 0..n_blocks {
          let mut sum = 0.0;
          for j in 0..block_sizes[i] {
            for ch in 0..block.num_channels() {
              sum += block.channel(ch)[b * block_sizes[i] + j].powi(2);
            }
          }
          peaks[i].push(Self::create_peak(
            sum,
            block_sizes[i] * block.num_channels(),
          ));
          n_total_blocks[i] += 1;
        }
      }

      if frames < BASE_BLOCK_SIZE {
        break;
      }
    }

    let mut pixel_to_s = Vec::with_capacity(self.mip_levels);
    for total in n_total_blocks {
      pixel_to_s.push(duration / total as f32);
    }
    self.render_peaks(peaks, pixel_to_s, duration)
  }

  fn create_peak(sum: f32, count: usize) -> f32 {
    assert!(count > 0);
    f32::sqrt(sum / count as f32)
  }

  fn render_peaks(
    &self,
    peaks: Vec<Vec<f32>>,
    pixel_to_s: Vec<f32>,
    duration: f32,
  ) -> Result<WaveformResult, UiError> {
    let mut levels = Vec::with_capacity(peaks.len());
    for level in peaks.iter() {
      let mut images = Vec::new();
      for chunk in level.chunks(MAX_TEXTURE_SIZE) {
        images.push(self.write_peaks_to_image(chunk)?);
      }
      levels.push(images);
    }
    Ok(WaveformResult {
      levels,
      pixel_to_s,
      duration,
    })
  }

  fn write_peaks_to_image(&self, peaks: &[f32]) -> Result<Vec<u8>, UiError> {
    let width: usize = peaks.len();
    let mut pixmap = Pixmap::new(width as u32, self.height as u32).ok_or(UiError::Audio(
      "Can't get pixmap to draw waveform".to_string(),
    ))?;
    pixmap.fill(Color::TRANSPARENT);
    let mut paint = Paint::default();
    paint.set_color_rgba8(0, 0, 0, 255);

    let center_y = (self.height as f32 / 2.0).floor();
    for (i, peak) in peaks.iter().enumerate() {
      assert!(!peak.is_nan());
      let peak = (*peak).clamp(0.05, 1.0);
      let start_y = (center_y - (peak * center_y)).floor();
      let end_y = (center_y + (peak * center_y)).floor();
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

const MAX_ENTRIES: usize = 32;

struct WaveformCacheEntry {
  level: usize,
  idx: usize,
  uri: String,
  bytes: Bytes,
}

pub struct WaveformCache {
  name: String,
  source: PathBuf,
  entries: LinkedList<WaveformCacheEntry>,
  info: Option<WaveformCacheInfo>,
}

impl WaveformCache {
  pub fn new(name: String, source: PathBuf) -> WaveformCache {
    WaveformCache {
      name,
      source,
      entries: Default::default(),
      info: None,
    }
  }

  /// Returns the number of mipmaps and the number of images per mipmap.
  pub fn info(&mut self) -> Result<&WaveformCacheInfo, UiError> {
    if self.info.is_none() {
      self.info = Some(WaveformResult::load_info_from_archive(&self.source)?);
    }

    Ok(self.info.as_ref().unwrap())
  }

  /// Loads the given mip level and image index into the cache, returning its
  /// data.
  pub fn load_entry(&mut self, level: usize, idx: usize) -> Result<(String, Bytes), UiError> {
    for entry in &self.entries {
      if entry.level == level && entry.idx == idx {
        return Ok((entry.uri.clone(), entry.bytes.clone()));
      }
    }

    let data = WaveformResult::load_img_from_archive(&self.source, level, idx)?;
    self.entries.push_front(WaveformCacheEntry {
      uri: format!("bytes://{}_mip_l{}_i{}.png", self.name, level, idx),
      level,
      idx,
      bytes: Bytes::from(data),
    });

    while self.entries.len() > MAX_ENTRIES {
      self.entries.pop_back();
    }

    let first = self.entries.front().unwrap();
    Ok((first.uri.clone(), first.bytes.clone()))
  }
}

enum WaveformState {
  Loading,
  Loaded(Arc<RwLock<WaveformCache>>),
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
      if let WaveformState::Loaded(cache) = &v {
        for entry in &cache.read().unwrap().entries {
          ctx.forget_image(&entry.uri);
        }
      }
    }

    self.waveforms.write().unwrap().clear();
  }

  pub fn get_waveform(
    &self,
    project_file: &File,
    event: &Event,
  ) -> Option<Arc<RwLock<WaveformCache>>> {
    if let Some(state) = self.waveforms.read().ok()?.get(&event.id) {
      return match state {
        WaveformState::Loading | WaveformState::Failed => None,
        WaveformState::Loaded(cache) => Some(cache.clone()),
      };
    }

    let cache_path = Cache::get_file_path(event.id, "tar").ok()?;
    if std::fs::exists(&cache_path).unwrap_or(false) {
      let cache = Arc::new(RwLock::new(WaveformCache::new(
        event.id.to_string(),
        cache_path,
      )));
      self
        .waveforms
        .write()
        .ok()?
        .insert(event.id, WaveformState::Loaded(cache.clone()));
      return Some(cache);
    }

    let resolver = KsngAttachmentResolver {};

    let audio_path = event.value.as_ref().and_then(|v| match v {
      EventValue::AudioClip { file, .. } => match &file.source {
        AudioFileSource::Path(path_buf) => Some(path_buf.clone()),
        AudioFileSource::Attachment(id) => {
          let attachment = project_file.attachments.iter().find(|a| a.id == *id)?;
          match &attachment.source {
            AttachmentSource::File(path_buf) => Some(path_buf.clone()),
            AttachmentSource::Managed => match resolver.read(attachment) {
              AttachmentReader::Path(path_buf) => Some(path_buf),
              AttachmentReader::Stream(_read) => todo!(),
            },
          }
        }
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
        if let Some(cache) = path {
          waveforms.insert(id, WaveformState::Loaded(cache));
        } else {
          waveforms.insert(id, WaveformState::Failed);
        }
      }
    });
  }

  fn load_waveform_impl(id: Uuid, path: PathBuf) -> Result<Arc<RwLock<WaveformCache>>, UiError> {
    let generator = WaveformGenerator {
      height: timeline::TRACK_HEIGHT as usize,
      mip_levels: 8,
    };
    let buffer = generator.create_waveform(path.as_ref())?;
    let cache_file = Cache::get_file_path(id, "tar")?;
    buffer.save_to_archive(&cache_file)?;
    Ok(Arc::new(RwLock::new(WaveformCache::new(
      id.to_string(),
      cache_file,
    ))))
  }
}
