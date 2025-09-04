use serde::{Deserialize, Serialize};

use crate::video::{
  elements::VideoElement,
  transitions::none::{NoneTransitionConfig, NoneTransitionElement},
};

pub mod none;

#[derive(Clone, Deserialize, Serialize)]
pub enum Transition {
  /// Lyrics will appear and disappear without transition.
  None(NoneTransitionConfig),
  Fade,
}

impl Default for Transition {
  fn default() -> Self {
    Transition::None(NoneTransitionConfig::default())
  }
}

pub fn apply_transition(
  transition: &Transition,
  elements: Vec<Box<dyn VideoElement>>,
) -> Vec<Box<dyn VideoElement>> {
  match transition {
    Transition::None(config) => NoneTransitionElement::wrap_elements(config, elements),
    Transition::Fade => todo!(),
  }
}
