use serde::{Deserialize, Serialize};

use crate::{
  timecode::Timecode,
  util::easing::EasingFunction,
  video::{
    elements::{VideoElement, VideoElementRenderContext},
    vacancy::VacancyChecker,
  },
};

#[derive(Clone, Serialize, Deserialize)]
pub struct SlideTransitionConfig {
  /// The maximum amount of time an event will be displayed for before it happens.
  pub lead_time: Timecode,
  /// The maximum amount of time an event will be displayed for after it happens.
  pub trail_time: Timecode,
  /// The maximum amount of time that the fade transition itself will play at the start.
  pub transition_time: Timecode,
  /// The easing function used for the slide in.
  pub easing_in: EasingFunction,
  /// The easing function used for the slide out.
  pub easing_out: EasingFunction,
}

impl Default for SlideTransitionConfig {
  fn default() -> Self {
    Self {
      lead_time: Timecode(2000),
      trail_time: Timecode(3000),
      transition_time: Timecode(500),
      easing_in: EasingFunction::QuadIn,
      easing_out: EasingFunction::QuadOut,
    }
  }
}

pub struct SlideTransitionElement {
  element: Box<dyn VideoElement>,
  start_time: Timecode,
  end_time: Timecode,
  transition_time: Timecode,
  easing_in: EasingFunction,
  easing_out: EasingFunction,
}

impl SlideTransitionElement {
  pub fn wrap_elements(
    config: &SlideTransitionConfig,
    elements: Vec<Box<dyn VideoElement>>,
  ) -> Vec<Box<dyn VideoElement>> {
    let vacancy = VacancyChecker::new(&elements);
    elements
      .into_iter()
      .map(|elem| {
        let (start_time, end_time) =
          vacancy.calc_start_end_midpoints(elem.as_ref(), config.lead_time, config.trail_time);

        Box::new(SlideTransitionElement {
          start_time,
          end_time,
          transition_time: config.transition_time,
          element: elem,
          easing_in: config.easing_in,
          easing_out: config.easing_out,
        }) as Box<dyn VideoElement>
      })
      .collect()
  }
}

impl VideoElement for SlideTransitionElement {
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
    let size = context.canvas.base_layer_size();
    let rect = skia_safe::Rect {
      left: 0.0,
      top: 0.0,
      right: size.width as f32,
      bottom: size.height as f32,
    };
    let clip_rect = if context.time < self.element.start_time() {
      let t = self.easing_in.evaluate(
        (context.time - self.start_time).to_seconds()
          / (self.element.start_time() - self.start_time)
            .min(self.transition_time)
            .to_seconds(),
      );

      skia_safe::Rect {
        left: rect.left,
        top: rect.top,
        right: rect.left + rect.width() * t,
        bottom: rect.bottom,
      }
    } else if context.time > self.element.end_time() {
      let t = 1.0
        - self.easing_out.evaluate(
          (self.end_time - context.time).to_seconds()
            / (self.end_time - self.element.end_time())
              .min(self.transition_time)
              .to_seconds(),
        );

      skia_safe::Rect {
        left: rect.left + rect.width() * t,
        top: rect.top,
        right: rect.right,
        bottom: rect.bottom,
      }
    } else {
      rect
    };

    context.canvas.save();
    context
      .canvas
      .clip_rect(clip_rect, skia_safe::ClipOp::Intersect, Some(true));

    self.element.render(context);

    context.canvas.restore();
  }
}
