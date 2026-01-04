use crate::{
  style::{Color32, Font},
  timecode::Timecode,
  Rect,
};

pub trait EditableConfig {
  fn edit(&mut self, ui: &mut dyn EditableConfigUi) -> bool;
}

/// Contains pointers to functions for rendering UI elements
/// for editing config.
///
/// Most methods takes at least a key (the name of the config item),
/// and a mutable boolean to specify that it has been changed.
///
/// If a blank string is specified for a key, the field will be rendered inline.
pub trait EditableConfigUi {
  /// Draws a checkbox.
  /// Params: (key, changed, value)
  fn checkbox(&mut self, key: &str, changed: &mut bool, value: bool) -> bool;
  /// Draws a slider.
  /// Params: (key, changed, min, max, value)
  fn slider(&mut self, key: &str, changed: &mut bool, min: f32, max: f32, value: f32) -> f32;
  /// Draws a float editor.
  /// Params: (key, changed, min?, max?, value)
  fn float(
    &mut self,
    key: &str,
    changed: &mut bool,
    min: Option<f32>,
    max: Option<f32>,
    value: f32,
  ) -> f32;
  /// Draws a integer editor.
  /// Params: (key, changed, min?, max?, value)
  fn integer(
    &mut self,
    key: &str,
    changed: &mut bool,
    min: Option<i64>,
    max: Option<i64>,
    value: i64,
  ) -> i64;
  /// Draws a normalized rect editor (all values from 0 to 1).
  /// Params: (key, changed, value)
  fn normalized_rect(&mut self, key: &str, changed: &mut bool, value: Rect) -> Rect;
  /// Draws an editor for the given EditableConfig.
  /// Params: (key, changed, new)
  fn config(&mut self, key: &str, changed: &mut bool, value: &mut dyn EditableConfig) -> ();
  /// Draws an inline dropdown.
  /// Params: (changed, options, current_option)
  fn dropdown<'a>(
    &mut self,
    key: &str,
    changed: &mut bool,
    options: &[&'a str],
    value: &'a str,
  ) -> &'a str;
  /// Draws a timecode editor.
  /// Params: (key, changed, value)
  fn timecode(&mut self, key: &str, changed: &mut bool, value: Timecode) -> Timecode;
  /// Draws a font picker.
  /// Params: (key, changed, value)
  fn font(&mut self, key: &str, changed: &mut bool, value: Font) -> Font;
  /// Draws a color editor.
  /// Params: (key, changed, value)
  fn color(&mut self, key: &str, changed: &mut bool, value: Color32) -> Color32;
}
