use vello::Scene;

use crate::timecode::Timecode;

pub mod paragraph;
pub mod solid;
pub mod text;

/// A video element is an renderable item with a position and a start and end time.
pub trait VideoElement {
  fn start_time(&self) -> Timecode;
  fn end_time(&self) -> Timecode;
  fn render(&self, scene: &mut Scene, time: Timecode);
}
