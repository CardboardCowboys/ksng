use crate::{
  style::{Color32, Font},
  timecode::Timecode,
  Rect,
};

pub trait EditableConfig {
  fn edit(&mut self, ui: &EditableConfigUi) -> bool;
}

/// Contains pointers to functions for rendering UI elements
/// for editing config.
///
/// Every method takes at least a key (the name of the config item),
/// and most take a mutable boolean to specify that it has been changed.
pub struct EditableConfigUi {
  /// Draws a checkbox.
  /// Params: (key, changed, value)
  pub checkbox: fn(&str, &mut bool, bool) -> bool,
  /// Draws a slider.
  /// Params: (key, changed, min, max, value)
  pub slider: fn(&str, &mut bool, f32, f32, f32) -> f32,
  /// Draws a number editor.
  /// Params: (key, changed, min?, max?, value)
  pub number: fn(&str, &mut bool, Option<f32>, Option<f32>, f32) -> f32,
  /// Draws a normalized rect editor (all values from 0 to 1).
  /// Params: (key, changed, value)
  pub normalized_rect: fn(&str, &mut bool, Rect) -> Rect,
  /// Draws a section.
  /// Params: (key, inner)
  pub section: fn(&str, fn() -> ()) -> (),
  /// Draws an editor for the given EditableConfig.
  /// Params: (key, inner, original, changed)
  pub config: fn(&str, &mut bool, Box<dyn EditableConfig>, &mut dyn EditableConfig) -> (),
  /// Draws a dropdown.
  /// Params: (key, changed, options)
  pub dropdown: for<'a> fn(&str, &mut bool, &[&'a str]) -> &'a str,
  /// Draws a timecode editor.
  /// Params: (key, changed, value)
  pub timecode: fn(&str, &mut bool, Timecode) -> Timecode,
  /// Draws a font picker.
  /// Params: (key, changed, value)
  pub font: fn(&str, &mut bool, Font) -> Font,
  /// Draws a color editor.
  /// Params: (key, changed, value)
  pub color: fn(&str, &mut bool, Color32) -> Color32,
}
