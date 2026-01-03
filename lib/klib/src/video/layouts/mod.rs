use klib_macros::EditableConfig;
use serde::{Deserialize, Serialize};

use crate::video::layouts::paragraph::ParagraphMergerMode;

pub mod paragraph;

#[derive(Serialize, Deserialize, Clone, EditableConfig)]
pub enum LyricsTrackLayoutMode {
  Paragraph { merger_mode: ParagraphMergerMode },
}
