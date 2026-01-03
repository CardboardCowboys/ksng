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
/// Most methods takes at least a key (the name of the config item),
/// and a mutable boolean to specify that it has been changed.
///
/// If a blank string is specified for a key, the field will be rendered inline.
pub struct EditableConfigUi {
  /// Draws a checkbox.
  /// Params: (key, changed, value)
  pub checkbox: fn(&str, &mut bool, bool) -> bool,
  /// Draws a slider.
  /// Params: (key, changed, min, max, value)
  pub slider: fn(&str, &mut bool, f32, f32, f32) -> f32,
  /// Draws a float editor.
  /// Params: (key, changed, min?, max?, value)
  pub float: fn(&str, &mut bool, Option<f32>, Option<f32>, f32) -> f32,
  /// Draws a integer editor.
  /// Params: (key, changed, min?, max?, value)
  pub integer: fn(&str, &mut bool, Option<i64>, Option<i64>, i64) -> i64,
  /// Draws a normalized rect editor (all values from 0 to 1).
  /// Params: (key, changed, value)
  pub normalized_rect: fn(&str, &mut bool, Rect) -> Rect,
  /// Draws a section.
  /// Params: (key, inner)
  pub section: fn(&str, fn() -> ()) -> (),
  /// Draws an editor for the given EditableConfig.
  /// Params: (key, changed, new)
  pub config: fn(&str, &mut bool, &mut dyn EditableConfig) -> (),
  /// Draws an inline dropdown.
  /// Params: (changed, options, current_option)
  pub dropdown: for<'a> fn(&str, &mut bool, &[&'a str], &'a str) -> &'a str,
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
