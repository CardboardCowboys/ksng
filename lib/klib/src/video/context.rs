use skia_safe::Arc;

use crate::{style::LyricsTrackStyle, Rect};

#[derive(Clone)]
pub struct LyricsTrackContext<'a> {
  pub area: Rect,
  pub style: &'a LyricsTrackStyle,
  pub font_mgr: skia_safe::FontMgr,
}
