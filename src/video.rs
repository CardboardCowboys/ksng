use egui::{load::SizedTexture, TextureFilter, TextureId, Vec2};
use klib::{
  objects::file::File,
  timecode::Timecode,
  video::{renderer::VideoRenderer, sequence::VideoSequence, VideoConfig},
};
use wgpu::TextureViewDescriptor;

use crate::util::error::UiError;

struct RenderedFrame {
  time: Timecode,
  texture_id: egui::TextureId,
  texture: wgpu::Texture,
}

pub struct VideoState {
  renderer: VideoRenderer,
  sequence: Option<VideoSequence>,
  video_config: VideoConfig,
  last_rendered_frame: Option<RenderedFrame>,
}

impl VideoState {
  pub fn new(device: &wgpu::Device) -> Result<VideoState, UiError> {
    Ok(VideoState {
      renderer: VideoRenderer::new(device)?,
      sequence: None,
      video_config: VideoConfig::default(),
      last_rendered_frame: None,
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

  pub fn process_frame(
    &mut self,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    wgpu_renderer: &mut egui_wgpu::Renderer,
    time: Timecode,
  ) -> Result<(), UiError> {
    /*if self
      .last_rendered_frame
      .as_ref()
      .map(|f| f.time == time)
      .unwrap_or(false)
    {
      // Frame already rendered.
      return Ok(());
    }*/

    let Some(sequence) = &self.sequence else {
      return Ok(());
    };

    let texture = self
      .renderer
      .render_frame(&self.video_config, device, queue, sequence, time)?;

    let texture_view = texture.create_view(&TextureViewDescriptor::default());

    if let Some(rendered_frame) = &self.last_rendered_frame {
      let texture_id = rendered_frame.texture_id;
      wgpu_renderer.update_egui_texture_from_wgpu_texture(
        device,
        &texture_view,
        wgpu::FilterMode::Linear,
        texture_id,
      );
      self.last_rendered_frame = Some(RenderedFrame {
        time,
        texture_id,
        texture,
      });
    } else {
      let texture_id =
        wgpu_renderer.register_native_texture(device, &texture_view, wgpu::FilterMode::Linear);
      self.last_rendered_frame = Some(RenderedFrame {
        time,
        texture_id,
        texture,
      });
    }

    Ok(())
  }

  pub fn last_frame_texture(&self) -> Option<SizedTexture> {
    self.last_rendered_frame.as_ref().map(|f| SizedTexture {
      id: f.texture_id,
      size: Vec2::new(
        self.video_config.width as f32,
        self.video_config.height as f32,
      ),
    })
  }
}
