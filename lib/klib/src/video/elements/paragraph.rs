use crate::{timecode::Timecode, video::elements::VideoElement};

pub struct ParagraphVideoElement {
  elements: Vec<Box<dyn VideoElement>>,
  start_time: Timecode,
  end_time: Timecode,
}

impl ParagraphVideoElement {
  pub fn from_elements<T: Iterator<Item = Box<dyn VideoElement>>>(
    elements: T,
  ) -> ParagraphVideoElement {
    let mut elements: Vec<Box<dyn VideoElement>> = elements.collect();
    elements.sort_by_key(|a| a.start_time());
    let start_time = elements
      .iter()
      .map(|e| e.start_time())
      .min()
      .unwrap_or(Timecode(0));
    let end_time = elements
      .iter()
      .map(|e| e.end_time())
      .max()
      .unwrap_or(Timecode(0));

    ParagraphVideoElement {
      elements,
      start_time,
      end_time,
    }
  }
}

impl VideoElement for ParagraphVideoElement {
  fn start_time(&self) -> Timecode {
    self.start_time
  }

  fn end_time(&self) -> Timecode {
    self.end_time
  }

  fn render(&self, scene: &mut vello::Scene, time: Timecode) {
    for element in &self.elements {
      element.render(scene, time);
    }
  }
}
