use serde::{Deserialize, Serialize};

use crate::{
  timecode::Timecode,
  video::{elements::VideoElement, vacancy::VacancyChecker},
};

#[derive(Clone, Serialize, Deserialize)]
pub struct NoneTransitionConfig {
  /// The maximum amount of time an event will be displayed for before it happens.
  pub lead_time: Timecode,
  /// The maximum amount of time an event will be displayed for after it happens.
  pub trail_time: Timecode,
}

impl Default for NoneTransitionConfig {
  fn default() -> Self {
    Self {
      lead_time: Timecode(2000),
      trail_time: Timecode(3000),
    }
  }
}

pub struct NoneTransitionElement {
  element: Box<dyn VideoElement>,
  start_time: Timecode,
  end_time: Timecode,
}

impl NoneTransitionElement {
  pub fn wrap_elements(
    config: &NoneTransitionConfig,
    elements: Vec<Box<dyn VideoElement>>,
  ) -> Vec<Box<dyn VideoElement>> {
    let vacancy = VacancyChecker::new(&elements);
    elements
      .into_iter()
      .map(|elem| {
        let last_end_time = vacancy.end_time_of_previous_occupant(elem.as_ref());
        let next_start_time = vacancy.start_time_of_following_occupant(elem.as_ref());

        Box::new(NoneTransitionElement {
          start_time: if let Some(last_end_time) = last_end_time {
            let diff = elem.start_time() - last_end_time;
            elem.start_time() - (diff / Timecode(2)).min(config.lead_time)
          } else {
            elem.start_time() - config.lead_time
          },
          end_time: if let Some(next_start_time) = next_start_time {
            let diff = next_start_time - elem.end_time();
            elem.end_time() + (diff / Timecode(2)).min(config.trail_time)
          } else {
            elem.end_time() + config.trail_time
          },
          element: elem,
        }) as Box<dyn VideoElement>
      })
      .collect()
  }
}

impl VideoElement for NoneTransitionElement {
  fn id(&self) -> uuid::Uuid {
    self.element.id()
  }

  fn start_time(&self) -> Timecode {
    self.start_time
  }

  fn end_time(&self) -> Timecode {
    self.end_time
  }

  fn bounds(&self) -> crate::Rect {
    self.element.bounds()
  }

  fn transform(&self) -> skia_safe::Matrix {
    self.element.transform()
  }

  fn set_transform(&mut self, mat: skia_safe::Matrix) {
    self.element.set_transform(mat);
  }

  fn render(&self, canvas: &skia_safe::Canvas, time: Timecode) {
    self.element.render(canvas, time);
  }
}
