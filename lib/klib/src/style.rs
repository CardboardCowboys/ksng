use parley::{fontique::FontInfo, FontContext};
use parley::{Brush, RangedBuilder, StyleProperty};
use serde::{Deserialize, Serialize};

/// A 32-bit RGBA color.
#[derive(Serialize, Deserialize, Copy, Clone)]
pub struct Color32(u32);

impl Color32 {
  pub fn from_rgb(r: u8, g: u8, b: u8) -> Color32 {
    Self::from_rgba(r, g, b, 0xff)
  }

  pub fn from_rgba(r: u8, g: u8, b: u8, a: u8) -> Color32 {
    Color32((r as u32) << 24 & (g as u32) << 16 & (b as u32) << 8 & (a as u32))
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

impl From<Color32> for vello::peniko::color::AlphaColor<vello::peniko::color::Srgb> {
  fn from(value: Color32) -> Self {
    vello::peniko::color::AlphaColor::<vello::peniko::color::Srgb>::new(value.to_floats())
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
  /// Oblique (or slanted) style with an optional angle in degrees,
  /// counter-clockwise from the vertical.
  Oblique(Option<f32>),
}

impl From<FontStyle> for parley::FontStyle {
  fn from(value: FontStyle) -> Self {
    match value {
      FontStyle::Normal => parley::FontStyle::Normal,
      FontStyle::Italic => parley::FontStyle::Italic,
      FontStyle::Oblique(angle) => parley::FontStyle::Oblique(angle),
    }
  }
}

/// Visual weight class of a font, typically on a scale from 1.0 to 1000.0.
///
/// The default value is [`FontWeight::NORMAL`] or `400.0`.
///
/// In variable fonts, this can be controlled with the `wght` [axis]. This
/// is an `f32` so that it can represent the same range of values as the
/// `wght` axis.
///
/// See <https://fonts.google.com/knowledge/glossary/weight>
///
/// In CSS, this corresponds to the [`font-weight`] property.
///
/// [axis]: crate::AxisInfo
/// [`font-weight`]: https://www.w3.org/TR/css-fonts-4/#font-weight-prop
#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, PartialOrd, Debug)]
pub struct FontWeight(f32);

impl From<FontWeight> for parley::FontWeight {
  fn from(value: FontWeight) -> Self {
    parley::FontWeight::new(value.0)
  }
}

/// Visual width of a font-- a relative change from the normal aspect
/// ratio, typically in the range `0.5` to `2.0`.
///
/// The default value is [`FontWidth::NORMAL`] or `1.0`.
///
/// In variable fonts, this can be controlled with the `wdth` [axis]. This
/// is an `f32` so that it can represent the same range of values as the
/// `wdth` axis.
///
/// In Open Type, the `u16` [`usWidthClass`] field has 9 values, from 1-9,
/// which doesn't allow for the wide range of values possible with variable
/// fonts.
///
/// See <https://fonts.google.com/knowledge/glossary/width>
///
/// In CSS, this corresponds to the [`font-width`] property.
///
/// This has also been known as "stretch" and has a legacy CSS name alias,
/// [`font-stretch`].
///
/// [axis]: crate::AxisInfo
/// [`usWidthClass`]: https://learn.microsoft.com/en-us/typography/opentype/spec/os2#uswidthclass
/// [`font-width`]: https://www.w3.org/TR/css-fonts-4/#font-width-prop
/// [`font-stretch`]: https://www.w3.org/TR/css-fonts-4/#font-stretch-prop
#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, PartialOrd, Debug)]
pub struct FontWidth(f32);

impl From<FontWidth> for parley::FontWidth {
  fn from(value: FontWidth) -> Self {
    parley::FontWidth::from_ratio(value.0)
  }
}

/// A font with configuration options.
#[derive(Serialize, Deserialize, Clone)]
pub struct Font {
  pub style: FontStyle,
  pub weight: FontWeight,
  pub width: FontWidth,
  pub family: String,
  pub size: f32,
}

impl Font {
  pub fn load_font_info(&self, font_context: &mut FontContext) -> Option<FontInfo> {
    let font_id = font_context.collection.family_id(&self.family)?;
    let family_info = font_context.collection.family(font_id)?;
    let font_info = family_info.match_font(
      self.width.into(),
      self.style.into(),
      self.weight.into(),
      true,
    )?;
    Some(font_info.clone())
  }

  pub fn push_builder<B: Brush>(&self, builder: &mut RangedBuilder<'_, B>) {
    let family = parley::FontFamily::Named(self.family.clone().into());
    builder.push_default(StyleProperty::FontStack(parley::FontStack::Single(family)));
    builder.push_default(StyleProperty::FontSize(self.size));
    builder.push_default(StyleProperty::FontStyle(self.style.into()));
    builder.push_default(StyleProperty::FontWeight(self.weight.into()));
    builder.push_default(StyleProperty::FontWidth(self.width.into()));
  }
}

impl Default for Font {
  fn default() -> Self {
    Self {
      style: FontStyle::Normal,
      weight: FontWeight(700.0),
      width: FontWidth(1.0),
      family: "Arial".to_string(),
      size: 16.0,
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
