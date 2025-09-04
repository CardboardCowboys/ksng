use crate::{
  error::Error,
  timecode::Timecode,
  video::{elements::VideoElementRenderContext, sequence::VideoSequence, VideoConfig},
};

pub struct VideoRenderer {}

impl VideoRenderer {
  /// Creates a new `VideoRenderer`.
  pub fn new() -> Result<VideoRenderer, Error> {
    Ok(VideoRenderer {})
  }

  /// Resizes the given buffer to fit the frame.
  pub fn allocate_buffer(video_config: &VideoConfig, buffer: &mut Vec<u8>) {
    let image_info = Self::image_info(video_config);
    buffer.resize(
      image_info.width() as usize * image_info.height() as usize * image_info.bytes_per_pixel(),
      0,
    );
  }

  /// Renders a frame of video at the given time to a buffer of RGBA8888 bytes.
  pub fn render_frame(
    &mut self,
    video_config: &VideoConfig,
    sequence: &VideoSequence,
    time: Timecode,
    buffer: &mut [u8],
  ) -> Result<(), Error> {
    let canvas =
      skia_safe::Canvas::from_raster_direct(&Self::image_info(video_config), buffer, None, None)
        .ok_or(Error::Skia("Failed to create Skia canvas".to_string()))?;

    canvas.clear(video_config.base_color);

    let mut scratch = canvas
      .new_surface(&canvas.image_info(), None)
      .ok_or(Error::Skia("Failed to create scratch surface".to_string()))?;

    let mut render_context = VideoElementRenderContext {
      time,
      canvas: &canvas,
      scratch_surface: Some(&mut scratch),
    };

    for element in sequence.elements_for_time(time) {
      element.render(&mut render_context);
    }

    Ok(())
  }

  fn resolution(video_config: &VideoConfig) -> skia_safe::ISize {
    skia_safe::ISize::new(video_config.width as i32, video_config.height as i32)
  }

  fn image_info(video_config: &VideoConfig) -> skia_safe::ImageInfo {
    skia_safe::ImageInfo::new(
      Self::resolution(video_config),
      skia_safe::ColorType::RGBA8888,
      skia_safe::AlphaType::Unpremul,
      None,
    )
  }
}
