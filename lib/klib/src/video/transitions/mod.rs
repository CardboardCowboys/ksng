use serde::{Deserialize, Serialize};

use crate::video::{
  elements::VideoElement,
  transitions::{
    fade::{FadeTransitionConfig, FadeTransitionElement},
    none::{NoneTransitionConfig, NoneTransitionElement},
    slide::{SlideTransitionConfig, SlideTransitionElement},
  },
};

pub mod fade;
pub mod none;
pub mod slide;

#[derive(Clone, Deserialize, Serialize)]
pub enum Transition {
  /// Lyrics will appear and disappear without transition.
  None(NoneTransitionConfig),
  /// Lyrics will fade in and out.
  Fade(FadeTransitionConfig),
  /// Lyrics will slide in and out.
  Slide(SlideTransitionConfig),
}

impl Default for Transition {
  fn default() -> Self {
    Transition::Fade(FadeTransitionConfig::default())
  }
}

pub fn apply_transition(
  transition: &Transition,
  elements: Vec<Box<dyn VideoElement>>,
) -> Vec<Box<dyn VideoElement>> {
  match transition {
    Transition::None(config) => NoneTransitionElement::wrap_elements(config, elements),
    Transition::Fade(config) => FadeTransitionElement::wrap_elements(config, elements),
    Transition::Slide(config) => SlideTransitionElement::wrap_elements(config, elements),
  }
}
