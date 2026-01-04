use klib_macros::EditableConfig;
use serde::{Deserialize, Serialize};
use skia_safe::Color4f;

use crate::{
  timecode::Timecode,
  util::easing::EasingFunction,
  video::{
    elements::{VideoElement, VideoElementRenderContext},
    vacancy::VacancyChecker,
  },
};

#[derive(Clone, Serialize, Deserialize, EditableConfig)]
pub struct FadeTransitionConfig {
  /// The maximum amount of time an event will be displayed for before it
  /// happens.
  pub lead_time: Timecode,
  /// The maximum amount of time an event will be displayed for after it
  /// happens.
  pub trail_time: Timecode,
  /// The maximum amount of time that the fade transition itself will play at
  /// the start.
  pub transition_time: Timecode,
  /// The easing function used for the fade in.
  pub easing_in: EasingFunction,
  /// The easing function used for the fade out.
  pub easing_out: EasingFunction,
}

impl Default for FadeTransitionConfig {
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

pub struct FadeTransitionElement {
  element: Box<dyn VideoElement>,
  start_time: Timecode,
  end_time: Timecode,
  transition_time: Timecode,
  easing_in: EasingFunction,
  easing_out: EasingFunction,
}

impl FadeTransitionElement {
  pub fn wrap_elements(
    config: &FadeTransitionConfig,
    elements: Vec<Box<dyn VideoElement>>,
  ) -> Vec<Box<dyn VideoElement>> {
    let vacancy = VacancyChecker::new(&elements);
    elements
      .into_iter()
      .map(|elem| {
        let (start_time, end_time) =
          vacancy.calc_start_end_midpoints(elem.as_ref(), config.lead_time, config.trail_time);

        Box::new(FadeTransitionElement {
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

impl VideoElement for FadeTransitionElement {
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
    let scratch_canvas = context.scratch_surface.as_mut().unwrap().canvas();
    scratch_canvas.clear(Color4f::new(0.0, 0.0, 0.0, 0.0));

    self.element.render(&mut VideoElementRenderContext {
      time: context.time,
      canvas: scratch_canvas,
      scratch_surface: None,
    });

    let t = if context.time < self.element.start_time() {
      self.easing_in.evaluate(
        (context.time - self.start_time).to_seconds()
          / (self.element.start_time() - self.start_time)
            .min(self.transition_time)
            .to_seconds(),
      )
    } else if context.time > self.element.end_time() {
      self.easing_out.evaluate(
        (self.end_time - context.time).to_seconds()
          / (self.end_time - self.element.end_time())
            .min(self.transition_time)
            .to_seconds(),
      )
    } else {
      1.0
    };

    let paint = skia_safe::Paint::new(Color4f::new(1.0, 1.0, 1.0, t), None);

    context.scratch_surface.as_mut().unwrap().draw(
      context.canvas,
      skia_safe::Point::default(),
      skia_safe::SamplingOptions::new(skia_safe::FilterMode::Linear, skia_safe::MipmapMode::Linear),
      Some(&paint),
    );
  }
}
