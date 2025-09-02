use crate::{
  objects::file::File,
  timecode::Timecode,
  video::{elements::VideoElement, layout_track, VideoConfig},
};

const LOOKUP_DURATION: Timecode = Timecode(1000);

/// A sequence contains a set of video elements and information about when to show them.
pub struct VideoSequence {
  elements: Vec<Box<dyn VideoElement>>,
  /// Contains the start and end index of the relevant events for every `LOOKUP_DURATION` ms in this sequence.
  lookup: Vec<(usize, usize)>,
  duration: Timecode,
}

impl VideoSequence {
  /// Creates a `VideoSequence` for a `File`.
  pub fn from_file(file: &File, video_config: &VideoConfig) -> VideoSequence {
    let mut elements = Vec::new();
    for track in &file.tracks {
      elements.extend(layout_track(track, video_config).into_iter());
    }

    elements.sort_by_key(|e| e.start_time());

    let duration = elements
      .iter()
      .map(|e| e.end_time())
      .max()
      .unwrap_or(Timecode(0));
    let num_lookups = duration.0.div_ceil(LOOKUP_DURATION.0) as usize;

    let mut lookup = Vec::with_capacity(num_lookups);
    for i in 0..num_lookups {
      let start = Timecode(LOOKUP_DURATION.0 * i as u32);
      let end = start + LOOKUP_DURATION;

      // TODO: we shouldn't need to re-enumerate the entire vec every loop

      let mut start_index = 0;
      let mut end_index = 0;
      for (j, element) in elements.iter().enumerate() {
        if element.start_time() > end {
          break;
        } else if element.end_time() < start {
          continue;
        } else if Timecode::ranges_overlap((element.start_time(), element.end_time()), (start, end))
        {
          if start_index == 0 {
            start_index = j;
          }
          end_index = j + 1;
        }
      }

      lookup.push((start_index, end_index));
    }

    VideoSequence {
      elements,
      lookup,
      duration,
    }
  }

  /// Returns all elements that are visible for the given timecode.
  pub fn elements_for_time(&self, time: Timecode) -> impl Iterator<Item = &Box<dyn VideoElement>> {
    self
      .lookup_elements_for_time(time)
      .filter(move |e| time >= e.start_time() && time < e.end_time())
  }

  fn lookup_elements_for_time(
    &self,
    time: Timecode,
  ) -> impl Iterator<Item = &Box<dyn VideoElement>> {
    if time > self.duration {
      #[allow(clippy::iter_skip_zero)]
      return self.elements.iter().skip(0).take(0);
    }
    let lookup_index = time.0.div_ceil(LOOKUP_DURATION.0) as usize;
    let (start_idx, end_idx) = self.lookup[lookup_index];
    self
      .elements
      .iter()
      .skip(start_idx)
      .take(end_idx - start_idx)
  }
}
