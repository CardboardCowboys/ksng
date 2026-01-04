use klib_macros::EditableConfig;
use serde::{Deserialize, Serialize};

use crate::{
  timecode::Timecode,
  video::{
    elements::{VideoElement, VideoElementRenderContext},
    vacancy::VacancyChecker,
  },
};

#[derive(Clone, Serialize, Deserialize, EditableConfig)]
pub struct NoneTransitionConfig {
  /// The maximum amount of time an event will be displayed for before it
  /// happens.
  pub lead_time: Timecode,
  /// The maximum amount of time an event will be displayed for after it
  /// happens.
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
        let (start_time, end_time) =
          vacancy.calc_start_end_midpoints(elem.as_ref(), config.lead_time, config.trail_time);

        Box::new(NoneTransitionElement {
          start_time,
          end_time,
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

  fn render<'canvas>(&self, context: &mut VideoElementRenderContext<'canvas>) {
    self.element.render(context);
  }
}
