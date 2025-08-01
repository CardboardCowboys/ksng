use std::num::NonZeroUsize;

use vello::{AaSupport, RendererOptions};

use crate::{
  error::Error,
  timecode::Timecode,
  video::{sequence::VideoSequence, VideoConfig},
};

pub struct VideoRenderer {
  renderer: vello::Renderer,
  video_config: VideoConfig,
}

impl VideoRenderer {
  /// Creates a new `VideoRenderer` from a wgpu device and a video config.
  pub fn new(device: &wgpu::Device, video_config: VideoConfig) -> Result<VideoRenderer, Error> {
    let options = RendererOptions {
      use_cpu: false,
      antialiasing_support: AaSupport::all(),
      num_init_threads: NonZeroUsize::new(1),
      pipeline_cache: None,
    };
    let renderer = vello::Renderer::new(device, options)?;
    Ok(VideoRenderer {
      renderer,
      video_config,
    })
  }

  /// Renders a frame of video at the given time to a wgpu texture.
  pub fn render_frame(
    &mut self,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    sequence: &VideoSequence,
    time: Timecode,
  ) -> Result<wgpu::Texture, Error> {
    let mut scene = vello::Scene::new();

    for element in sequence.elements_for_time(time) {
      element.render(&mut scene, time);
    }

    let texture = device.create_texture(&wgpu::TextureDescriptor {
      label: Some("klib::VideoRenderer::render_frame"),
      size: wgpu::Extent3d {
        width: self.video_config.width as u32,
        height: self.video_config.height as u32,
        depth_or_array_layers: 0,
      },
      mip_level_count: 1,
      sample_count: 1,
      dimension: wgpu::TextureDimension::D2,
      format: wgpu::TextureFormat::Rgba8UnormSrgb,
      usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
      view_formats: &[],
    });

    let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    self.renderer.render_to_texture(
      device,
      queue,
      &scene,
      &texture_view,
      &vello::RenderParams {
        base_color: self.video_config.base_color.into(),
        width: self.video_config.width as u32,
        height: self.video_config.height as u32,
        antialiasing_method: vello::AaConfig::Area,
      },
    )?;

    Ok(texture)
  }
}
