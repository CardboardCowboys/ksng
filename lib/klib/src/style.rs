use serde::{Deserialize, Serialize};

use crate::util::editable_config::{EditableConfig, EditableConfigUi};

/// A 32-bit RGBA color.
#[derive(Serialize, Deserialize, Copy, Clone)]
pub struct Color32(u32);

impl Color32 {
  pub fn from_rgb(r: u8, g: u8, b: u8) -> Color32 {
    Self::from_rgba(r, g, b, 0xff)
  }

  pub fn from_rgba(r: u8, g: u8, b: u8, a: u8) -> Color32 {
    Color32((r as u32) << 24 | (g as u32) << 16 | (b as u32) << 8 | (a as u32))
  }

  pub fn to_floats(self) -> [f32; 4] {
    let r = self.0 >> 24 & 0xff;
    let g = self.0 >> 16 & 0xff;
    let b = self.0 >> 8 & 0xff;
    let a = self.0 & 0xff;
    [
      r as f32 / 0xff as f32,
      g as f32 / 0xff as f32,
      b as f32 / 0xff as f32,
      a as f32 / 0xff as f32,
    ]
  }
}

impl From<Color32> for skia_safe::Color4f {
  fn from(value: Color32) -> Self {
    let floats = value.to_floats();
    skia_safe::Color4f::new(floats[0], floats[1], floats[2], floats[3])
  }
}

/// We have to copy all the parley font structs so they can be serde'd

#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Default, Debug)]
pub enum FontStyle {
  /// An upright or "roman" style.
  #[default]
  Normal,
  /// Generally a slanted style, originally based on semi-cursive forms.
  /// This often has a different structure from the normal style.
  Italic,
  Oblique,
}

impl From<FontStyle> for skia_safe::font_style::Slant {
  fn from(value: FontStyle) -> Self {
    match value {
      FontStyle::Normal => skia_safe::font_style::Slant::Upright,
      FontStyle::Italic => skia_safe::font_style::Slant::Italic,
      FontStyle::Oblique => skia_safe::font_style::Slant::Oblique,
    }
  }
}

/// Visual weight class of a font, typically on a scale from 1 to 1000.
#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, PartialOrd, Debug)]
pub struct FontWeight(i32);

impl From<FontWeight> for skia_safe::font_style::Weight {
  fn from(value: FontWeight) -> Self {
    skia_safe::font_style::Weight::from(value.0)
  }
}

/// Visual width of a font
#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, PartialOrd, Debug)]
pub struct FontWidth(i32);

impl From<FontWidth> for skia_safe::font_style::Width {
  fn from(value: FontWidth) -> Self {
    skia_safe::font_style::Width::from(value.0)
  }
}

/// A font with configuration options.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Font {
  pub style: FontStyle,
  pub weight: FontWeight,
  pub width: FontWidth,
  pub family: String,
  pub size: f32,
}

impl Font {
  pub fn to_skfont(&self, font_mgr: &skia_safe::FontMgr) -> Option<skia_safe::Font> {
    let style = skia_safe::FontStyle::new(self.weight.into(), self.width.into(), self.style.into());
    let typeface = font_mgr.match_family_style(&self.family, style)?;
    Some(skia_safe::Font::new(typeface, Some(self.size)))
  }
}

impl Default for Font {
  fn default() -> Self {
    Self {
      style: FontStyle::Normal,
      weight: FontWeight(700),
      width: FontWidth(5),
      family: "Arial".to_string(),
      size: 100.0,
    }
  }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Colors {
  pub normal: Color32,
  pub highlight: Color32,
  pub background: Color32,
}

impl Default for Colors {
  fn default() -> Self {
    Self {
      normal: Color32::from_rgb(255, 255, 255),
      highlight: Color32::from_rgb(70, 175, 90),
      background: Color32::from_rgb(230, 230, 230),
    }
  }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Stroke {
  pub color: Color32,
  pub width: f32,
}

impl Default for Stroke {
  fn default() -> Self {
    Self {
      color: Color32::from_rgb(0, 0, 0),
      width: 3.0,
    }
  }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct LyricsTrackStyle {
  pub font: Font,
  pub colors: Colors,
  pub stroke: Stroke,
  pub line_height_multiplier: f32,
}

impl Default for LyricsTrackStyle {
  fn default() -> Self {
    Self {
      font: Default::default(),
      colors: Default::default(),
      stroke: Default::default(),
      line_height_multiplier: 1.0,
    }
  }
}
