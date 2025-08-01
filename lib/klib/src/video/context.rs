use parley::{FontContext, LayoutContext};

use crate::{style::LyricsTrackStyle, Rect};

#[derive(Clone)]
pub struct LyricsTrackContext<'a> {
  pub area: Rect,
  pub style: &'a LyricsTrackStyle,
  pub font_context: FontContext,
  pub layout_context: LayoutContext,
}
