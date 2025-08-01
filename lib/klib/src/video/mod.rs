use parley::{FontContext, LayoutContext};

use crate::{
  objects::track::{LyricsTrackValue, Track, TrackType, TrackValue},
  style::Color32,
  video::{
    context::LyricsTrackContext,
    elements::VideoElement,
    layouts::{paragraph::ParagraphLayout, LyricsTrackLayoutMode},
  },
  Rect,
};

pub mod context;
pub mod elements;
pub mod layouts;
pub mod renderer;
pub mod sequence;

pub struct VideoConfig {
  pub width: f64,
  pub height: f64,
  pub base_color: Color32,
}

fn layout_lyrics_track(track: &Track, video_config: &VideoConfig) -> Vec<Box<dyn VideoElement>> {
  let default_track_config = LyricsTrackValue::default();
  let track_config = match &track.track_value {
    Some(TrackValue::Lyrics(track_value)) => track_value,
    _ => &default_track_config,
  };

  let mut context = LyricsTrackContext {
    area: Rect {
      x0: video_config.width * track_config.bounds.x0,
      y0: video_config.height * track_config.bounds.y0,
      x1: video_config.width * track_config.bounds.x1,
      y1: video_config.height * track_config.bounds.y1,
    },
    style: &track_config.style,
    font_context: FontContext::new(),
    layout_context: LayoutContext::new(),
  };

  match track_config.layout {
    LyricsTrackLayoutMode::Paragraph { merger_mode } => {
      ParagraphLayout::new(merger_mode).layout_track(&mut context, track)
    }
  }
}

/// Turns a track into a set of video elements that can be rendered.
pub fn layout_track(track: &Track, video_config: &VideoConfig) -> Vec<Box<dyn VideoElement>> {
  match track.track_type {
    TrackType::Lyrics => layout_lyrics_track(track, video_config),
    TrackType::Audio => vec![],
  }
}
