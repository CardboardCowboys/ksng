use uuid::Uuid;

use crate::{timecode::Timecode, video::elements::VideoElement, Rect};

struct VacancyCheckerEntry {
  element_id: Uuid,
  rect: Rect,
  start_time: Timecode,
  end_time: Timecode,
}

/// Handles evaluating what other elements an element should be concerned with,
/// for purposes of transitions.
pub struct VacancyChecker {
  element_rects: Vec<VacancyCheckerEntry>,
}

impl VacancyChecker {
  /// Creates a new `VacancyChecker` from a set of elements.
  /// `elements` should be already sorted by start time.
  pub fn new(elements: &[Box<dyn VideoElement>]) -> VacancyChecker {
    // TODO: only works if events are non-overlapping. Is this a requirement?
    let mut rects = Vec::with_capacity(elements.len());
    for element in elements {
      let rect: skia_safe::Rect = element.bounds().into();
      let (transformed_rect, _) = element.transform().map_rect(rect);
      rects.push(VacancyCheckerEntry {
        element_id: element.id(),
        rect: transformed_rect.into(),
        start_time: element.start_time(),
        end_time: element.end_time(),
      });
    }

    VacancyChecker {
      element_rects: rects,
    }
  }

  /// Returns the end time of the first previous element to the provided element, if any.
  pub fn end_time_of_previous_occupant(&self, element: &dyn VideoElement) -> Option<Timecode> {
    let element_id = element.id();
    let (index, entry) = self
      .element_rects
      .iter()
      .enumerate()
      .find(|(_, entry)| entry.element_id == element_id)?;

    if index == 0 {
      return None;
    }

    let element_rect = &entry.rect;
    for i in (0..index).rev() {
      if self.element_rects[i].rect.intersects(element_rect) {
        return Some(self.element_rects[i].end_time);
      }
    }

    None
  }

  /// Returns the start time of the first following element to the provided element, if any.
  pub fn start_time_of_following_occupant(&self, element: &dyn VideoElement) -> Option<Timecode> {
    let element_id = element.id();
    let (index, entry) = self
      .element_rects
      .iter()
      .enumerate()
      .find(|(_, entry)| entry.element_id == element_id)?;

    if index == self.element_rects.len() - 1 {
      return None;
    }

    let element_rect = &entry.rect;
    for i in (index + 1)..self.element_rects.len() {
      if self.element_rects[i].rect.intersects(element_rect) {
        return Some(self.element_rects[i].start_time);
      }
    }

    None
  }

  /// Calculates the start and end times of an element given the specified lead and trail times.
  ///
  /// If there are elements within the span of `(start_time - lead_time, end_time + trail_time)` that
  /// overlap this element, the start and end times will be set to the midpoint between its time and
  /// the conflicting element's time. Otherwise, it will be the maximum value.
  pub fn calc_start_end_midpoints(
    &self,
    elem: &dyn VideoElement,
    lead_time: Timecode,
    trail_time: Timecode,
  ) -> (Timecode, Timecode) {
    let last_end_time = self.end_time_of_previous_occupant(elem);
    let next_start_time = self.start_time_of_following_occupant(elem);
    (
      if let Some(last_end_time) = last_end_time {
        let diff = elem.start_time() - last_end_time;
        elem.start_time() - (diff / Timecode(2)).min(lead_time)
      } else {
        elem.start_time() - lead_time
      },
      if let Some(next_start_time) = next_start_time {
        let diff = next_start_time - elem.end_time();
        elem.end_time() + (diff / Timecode(2)).min(trail_time)
      } else {
        elem.end_time() + trail_time
      },
    )
  }
}
