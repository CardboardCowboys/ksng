use skia_safe::Matrix;
use uuid::Uuid;

use crate::{timecode::Timecode, Rect};

pub mod paragraph;
pub mod text;

/// A video element is an renderable item with a position and a start and end time.
pub trait VideoElement {
  /// The ID of this video element.
  ///
  /// This might be equivalent to the ID of an event, or may be an entirely unique ID.
  fn id(&self) -> Uuid;
  fn start_time(&self) -> Timecode;
  fn end_time(&self) -> Timecode;
  fn bounds(&self) -> Rect;
  fn transform(&self) -> Matrix;
  fn set_transform(&mut self, mat: Matrix);
  fn render(&self, canvas: &skia_safe::Canvas, time: Timecode);
}
