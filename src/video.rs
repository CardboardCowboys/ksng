use egui::{load::SizedTexture, TextureHandle, TextureOptions};
use klib::{
  objects::file::File,
  timecode::Timecode,
  video::{renderer::VideoRenderer, sequence::VideoSequence, VideoConfig},
};

use crate::util::error::UiError;

struct RenderedFrame {
  time: Timecode,
  texture_handle: TextureHandle,
}

pub struct VideoState {
  renderer: VideoRenderer,
  sequence: Option<VideoSequence>,
  video_config: VideoConfig,
  last_rendered_frame: Option<RenderedFrame>,
  buffer: Vec<u8>,
}

impl VideoState {
  pub fn new() -> Result<VideoState, UiError> {
    Ok(VideoState {
      renderer: VideoRenderer::new()?,
      sequence: None,
      video_config: VideoConfig::default(),
      last_rendered_frame: None,
      buffer: Vec::new(),
    })
  }

  pub fn update_from_file(&mut self, file: &File) {
    self.video_config = file.config.video.clone();
    self.sequence = Some(VideoSequence::from_file(file, &self.video_config));
    self.last_rendered_frame = None;
  }

  pub fn clear(&mut self) {
    self.video_config = Default::default();
    self.sequence = None;
    self.last_rendered_frame = None;
  }

  pub fn process_frame(&mut self, context: &egui::Context, time: Timecode) -> Result<(), UiError> {
    if self
      .last_rendered_frame
      .as_ref()
      .map(|f| f.time == time)
      .unwrap_or(false)
    {
      // Frame already rendered.
      return Ok(());
    }

    let Some(sequence) = &self.sequence else {
      return Ok(());
    };

    VideoRenderer::allocate_buffer(&self.video_config, &mut self.buffer);

    self
      .renderer
      .render_frame(&self.video_config, sequence, time, &mut self.buffer)?;

    let color_image = egui::ColorImage::from_rgba_unmultiplied(
      [
        self.video_config.width as usize,
        self.video_config.height as usize,
      ],
      &self.buffer,
    );

    let handle = match &mut self.last_rendered_frame {
      None => context.load_texture("video#frame_tex", color_image, TextureOptions::LINEAR),
      Some(frame) => {
        frame
          .texture_handle
          .set(color_image, TextureOptions::LINEAR);
        frame.texture_handle.clone()
      }
    };

    self.last_rendered_frame = Some(RenderedFrame {
      time,
      texture_handle: handle,
    });

    Ok(())
  }

  pub fn last_frame_texture(&self) -> Option<SizedTexture> {
    self.last_rendered_frame.as_ref().map(|f| SizedTexture {
      id: f.texture_handle.id(),
      size: f.texture_handle.size_vec2(),
    })
  }
}
