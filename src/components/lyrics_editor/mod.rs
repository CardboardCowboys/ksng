use std::{collections::HashSet, fmt::Write};

use egui::{Button, FontId, MenuBar, Sides, Ui};
use klib::{
  objects::{
    event::{Event, EventType},
    track::{Track, TrackType},
  },
  timecode::Timecode,
};
use uuid::Uuid;

use crate::{
  KsngApp,
  commands::track::ApplyLyricsTrackChangesCommand,
  components::lyrics_editor::{psuedo_event::PsuedoEvent, text_element::LyricsEditorTextElement},
  project::Project,
  style::icons,
};

mod psuedo_event;
mod text_element;

pub const ESCAPE_CHAR: char = '\\';
pub const SYLLABLE_SEPARATOR: char = '-';
pub const BLOCK_OPEN: char = '[';
pub const BLOCK_CLOSE: char = ']';

struct LyricsEditorText {
  elements: Vec<LyricsEditorTextElement>,
  text: String,
  changed: bool,
  track_id: Uuid,
}

impl LyricsEditorText {
  pub fn new(track_id: Uuid, elements: Vec<LyricsEditorTextElement>) -> Self {
    let mut s = String::new();
    let mut last_elem_lyric = false;
    for e in &elements {
      if last_elem_lyric && e.event_type == EventType::Lyric {
        let _ = write!(&mut s, " ");
      }
      let _ = write!(&mut s, "{}", e);
      last_elem_lyric = e.event_type == EventType::Lyric;
    }

    LyricsEditorText {
      elements,
      text: s,
      changed: false,
      track_id,
    }
  }
}

#[derive(Default)]
struct LyricsData {
  tracks: Vec<LyricsEditorText>,
  selected_lyrics_track: usize,
}

#[derive(Default)]
pub struct LyricsEditor {
  current_data: Option<LyricsData>,
}

impl LyricsEditor {
  pub fn on_project_change(&mut self, app: &KsngApp) {
    self.on_lyrics_change(app);
    if let Some(data) = &mut self.current_data {
      data.selected_lyrics_track = 0;
    }
  }

  pub fn on_lyrics_change(&mut self, app: &KsngApp) {
    match app.project.borrow().as_ref() {
      Some(project) => self.rebuild_lyrics(project),
      None => self.current_data = None,
    };
  }

  fn rebuild_lyrics(&mut self, project: &Project) {
    if self.current_data.is_none() {
      self.current_data.replace(LyricsData::default());
    }

    let Some(data) = &mut self.current_data else {
      return;
    };

    let was_selected_id = if data.selected_lyrics_track < data.tracks.len() {
      Some(data.tracks[data.selected_lyrics_track].track_id)
    } else {
      None
    };

    data.tracks.retain(|t| t.changed);
    let remaining_ids: HashSet<Uuid> = HashSet::from_iter(data.tracks.iter().map(|t| t.track_id));

    for track in &project.file.tracks {
      if track.track_type != TrackType::Lyrics {
        continue;
      }

      // this track is modified and we didn't remove it
      if remaining_ids.contains(&track.id) {
        continue;
      }

      let mut elements = Vec::new();
      let mut current_events = Vec::new();
      let mut last_lyric_id = Uuid::default();

      for ev in track.events.iter() {
        if ev.event_type == EventType::Lyric
          && let Some(linked_id) = ev.linked_id
          && linked_id == last_lyric_id
        {
          current_events.push(ev.clone());
          last_lyric_id = ev.id;
          continue;
        }

        if !current_events.is_empty() {
          elements.push(LyricsEditorTextElement::new(
            EventType::Lyric,
            current_events,
          ));
          current_events = Vec::new();
          last_lyric_id = Uuid::default();
        }

        if ev.event_type == EventType::Lyric {
          current_events.push(ev.clone());
          last_lyric_id = ev.id;
        } else {
          elements.push(LyricsEditorTextElement::new(
            ev.event_type,
            vec![ev.clone()],
          ));
        }
      }

      if !current_events.is_empty() {
        elements.push(LyricsEditorTextElement::new(
          EventType::Lyric,
          current_events,
        ));
      }

      data.tracks.push(LyricsEditorText::new(track.id, elements));
    }

    if let Some(was_selected_id) = was_selected_id
      && let Some((idx, _)) = data
        .tracks
        .iter()
        .enumerate()
        .find(|(_, t)| t.track_id == was_selected_id)
    {
      data.selected_lyrics_track = idx;
    }
  }

  pub fn show(&mut self, app: &KsngApp, ui: &mut Ui) {
    let mut apply_changes = false;
    egui::TopBottomPanel::top("lyrics_editor#top").show_inside(ui, |ui| {
      MenuBar::new().ui(ui, |ui| {
        let track_changed = self
          .current_data
          .as_ref()
          .map(|d| !d.tracks.is_empty() && d.tracks[d.selected_lyrics_track].changed)
          .unwrap_or(false);

        Sides::new().show(
          ui,
          |ui| {
            // If there aren't any tracks available, we just want to show "No lyrics tracks"
            let has_lyrics_tracks = self
              .current_data
              .as_ref()
              .map(|d| !d.tracks.is_empty())
              .unwrap_or_default();
            ui.add_enabled_ui(has_lyrics_tracks, |ui| match &mut self.current_data {
              Some(data) => {
                let current_track_name =
                  format!("Lyrics Track #{}", data.selected_lyrics_track + 1);
                let mut selected_lyrics_track = data.selected_lyrics_track;
                ui.menu_button(current_track_name, |ui| {
                  for i in 0..data.tracks.len() {
                    if ui.button(format!("Lyrics Track #{}", i + 1)).clicked() {
                      selected_lyrics_track = i;
                    }
                  }
                });
                data.selected_lyrics_track = selected_lyrics_track;
              }
              None => {
                ui.menu_button("No lyrics tracks", |_ui| {});
              }
            });
          },
          |ui| {
            ui.add_enabled_ui(track_changed, |ui| {
              if ui
                .add(Button::image_and_text(icons::CHECKS, "Apply"))
                .clicked()
              {
                apply_changes = true;
              }
            })
          },
        );
      });
    });

    egui::CentralPanel::default().show_inside(ui, |ui| {
      egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
        let Some(data) = &mut self.current_data else {
          return;
        };

        let lyrics_track = &mut data.tracks[data.selected_lyrics_track];

        /*
        let text_copy = lyrics_track.text.clone();
        let mut layouter = |ui: &egui::Ui, buf: &dyn egui::TextBuffer, wrap_width: f32| {
          let mut layout_job: egui::text::LayoutJob = Self::layout_lyrics(text_copy, ui);
          layout_job.wrap.max_width = wrap_width;
          ui.fonts(|f| f.layout_job(layout_job))
        };*/

        let output = egui::TextEdit::multiline(&mut lyrics_track.text)
          .frame(false)
          .desired_width(ui.available_width() - 20.0)
          .font(FontId::proportional(20.0))
          //.layouter(&mut layouter)
          .show(ui);

        if output.response.changed() {
          lyrics_track.changed = true;
        }
      });
    });

    if apply_changes
      && let Some(data) = &mut self.current_data
      && data.selected_lyrics_track < data.tracks.len()
      && let Some(project) = app.project.borrow().as_ref()
    {
      let lyrics_track = &mut data.tracks[data.selected_lyrics_track];
      let Some(track) = project
        .file
        .tracks
        .iter()
        .find(|t| t.id == lyrics_track.track_id)
      else {
        return;
      };

      let new_events =
        Self::apply_changes_to_events(track, &lyrics_track.elements, &lyrics_track.text);

      app
        .commands
        .dispatch(ApplyLyricsTrackChangesCommand::new(track.id, new_events));

      lyrics_track.changed = false;
    }
  }

  /*fn layout_lyrics(text: &String, ui: &Ui) -> LayoutJob {
    let mut job = LayoutJob {
      text: text.clone(),
      ..Default::default()
    };

    // TODO: highlight text based on playback position
    job.sections.push(LayoutSection {
      leading_space: 0.0,
      byte_range: 0..job.text.len(),
      format: TextFormat {
        font_id: FontId::proportional(20.0),
        color: Color32::WHITE,
        italics: false,
        ..Default::default()
      },
    });

    job
  }*/

  fn apply_changes_to_events(
    track: &Track,
    old: &[LyricsEditorTextElement],
    new: &str,
  ) -> Vec<Event> {
    let old_events: Vec<PsuedoEvent> = old.iter().flat_map(|e| e.events.iter().cloned()).collect();
    let new_events = PsuedoEvent::from_str(new);
    let diff = similar::capture_diff_slices(similar::Algorithm::Patience, &old_events, &new_events);

    let mut out_events = Vec::new();
    // we're going to go through and handle linking events afterwards
    let mut need_to_be_linked = HashSet::new();

    for op in diff {
      match op {
        similar::DiffOp::Equal {
          old_index,
          new_index,
          len,
        } => {
          for i in 0..len {
            let Some(old_ev) = track
              .events
              .iter()
              .find(|e| e.id == old_events[old_index + i].id)
            else {
              continue;
            };
            let mut new_ev = old_ev.clone();
            if !new_events[new_index + i].linked_to_prev {
              new_ev.linked_id = None;
            } else {
              need_to_be_linked.insert(new_ev.id);
            }
            out_events.push(new_ev);
          }
        }
        similar::DiffOp::Delete { .. } => {
          // we deleted these events so we can do nothing!
        }
        similar::DiffOp::Replace {
          old_index,
          old_len,
          new_index,
          new_len,
        } => {
          // replace the old events with new events taking up the same duration
          let old_start = old_events[old_index].start_timecode;
          let old_end = old_events[old_index + old_len - 1].end_timecode;
          let prev_duration_per_event =
            Timecode(((old_end - old_start).0 as f64 / old_len as f64) as u32);
          let duration_per_event = if old_len >= new_len {
            // old events covered an equal or greater span than these, just use that span
            Timecode(((old_end - old_start).0 as f64 / new_len as f64) as u32)
          } else if (old_index + old_len) < old_events.len() {
            // there is a next event, we can use the span until then (if less than max)
            Timecode(
              ((old_events[old_index + old_len].start_timecode - old_start).0 as f64
                / new_len as f64) as u32,
            )
            .min(prev_duration_per_event)
          } else {
            // no next event - use as much time per event as the previous span
            prev_duration_per_event
          };
          let mut current_start = old_start;
          for e in new_events.iter().skip(new_index).take(new_len) {
            let new_ev = e.to_event(current_start, current_start + duration_per_event);
            out_events.push(new_ev);
            if e.linked_to_prev {
              need_to_be_linked.insert(e.id);
            }
            current_start += duration_per_event;
          }
        }
        similar::DiffOp::Insert {
          old_index,
          new_index,
          new_len,
        } => {
          // old_index is the index we're inserting to, the previous event is old_index -
          // 1
          let mut current_start = if (old_index - 1) < old_events.len() && (old_index > 0) {
            old_events[old_index - 1].end_timecode
          } else {
            Timecode(0)
          };
          let duration_per_event = if old_index < old_events.len() {
            Timecode(
              ((old_events[old_index].start_timecode - current_start).0 as f64 / new_len as f64)
                as u32,
            )
          } else {
            Timecode(100)
          };

          for e in new_events.iter().skip(new_index).take(new_len) {
            let new_ev = e.to_event(current_start, current_start + duration_per_event);
            out_events.push(new_ev);
            if e.linked_to_prev {
              need_to_be_linked.insert(e.id);
            }
            current_start += duration_per_event;
          }
        }
      }
    }

    // fix up and apply linking
    let mut last_lyric_id = None;
    let mut last_end = Timecode(0);
    for ev in &mut out_events {
      if ev.event_type == EventType::Lyric {
        if let Some(last_lyric_id) = last_lyric_id
          && need_to_be_linked.contains(&ev.id)
        {
          ev.linked_id = Some(last_lyric_id);
        }

        last_lyric_id = Some(ev.id);
      } else {
        last_lyric_id = None;
      }

      let duration = ev.end_timecode - ev.start_timecode;

      if ev.start_timecode < last_end {
        ev.start_timecode = last_end;
      }

      if (ev.end_timecode - ev.start_timecode) < duration {
        ev.end_timecode = ev.start_timecode + duration;
      }

      last_end = ev.end_timecode;
    }

    out_events
  }
}
