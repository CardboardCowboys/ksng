use egui::{
  Button, Color32, FontId, Id, Key, Sides, TextFormat,
  text::{CCursor, LayoutJob, LayoutSection},
};
use klib::{
  objects::{
    event::{Event, EventType, EventValue},
    track::Track,
  },
  timecode::Timecode,
};
use uuid::Uuid;

use crate::{
  KsngApp, commands::event::SetEventTimingsCommand, modals::confirm::ConfirmModal,
  util::ui_event::KsngEvent, windows::KWindow,
};

pub struct SyncWindow {
  open: bool,
  should_request_focus: bool,
  track_id: Uuid,
  layout_job: Option<LayoutJob>,
  layout_cursor: CCursor,
  current_idx: usize,
  last_idx: Option<usize>,
  finished_last_syllable: bool,
  event_ids: Vec<Uuid>,
  event_timings: Vec<(Timecode, Timecode)>,
  events_need_repositioning: Vec<usize>,
  undo_context: Vec<(usize, Option<usize>, Timecode, Timecode)>,
  min_time: Timecode,
  pending_scroll: bool,
  unique_value: u64,
  is_dirty: bool,
}

impl SyncWindow {
  pub fn new(track_id: Uuid) -> SyncWindow {
    SyncWindow {
      open: true,
      should_request_focus: false,
      track_id,
      layout_job: None,
      layout_cursor: CCursor::new(0),
      current_idx: 0,
      last_idx: None,
      finished_last_syllable: true,
      event_ids: Vec::new(),
      event_timings: Vec::new(),
      events_need_repositioning: Vec::new(),
      undo_context: Vec::new(),
      min_time: Timecode(0),
      pending_scroll: true,
      unique_value: egui::util::hash("sync_window"),
      is_dirty: false,
    }
  }

  fn layout_lyrics(track: Option<&Track>, current_idx: usize) -> (LayoutJob, CCursor) {
    let Some(track) = track else {
      return (LayoutJob::default(), CCursor::new(0));
    };

    let mut s = String::new();
    let mut needs_space = false;
    for idx in 0..current_idx {
      if let Some(ev) = track.events.get(idx) {
        Self::add_event_to_string(&mut s, ev, &mut needs_space);
      }
    }

    let before_len = s.len();
    if current_idx < track.events.len()
      && let Some(ev) = track.events.get(current_idx)
    {
      Self::add_event_to_string(&mut s, ev, &mut needs_space);
    }

    let current_len = s.len() - before_len;
    let cursor = CCursor::new(s.len());
    let final_start = s.len();

    if current_idx < track.events.len() {
      for idx in (current_idx + 1)..track.events.len() {
        if let Some(ev) = track.events.get(idx) {
          Self::add_event_to_string(&mut s, ev, &mut needs_space);
        }
      }
    }

    let mut job = LayoutJob {
      text: s,
      ..Default::default()
    };

    if before_len > 0 {
      job.sections.push(LayoutSection {
        leading_space: 0.0,
        byte_range: 0..before_len,
        format: TextFormat {
          font_id: FontId::proportional(20.0),
          color: Color32::GRAY,
          ..Default::default()
        },
      });
    }

    if current_len > 0 {
      job.sections.push(LayoutSection {
        leading_space: 0.0,
        byte_range: before_len..(before_len + current_len),
        format: TextFormat {
          font_id: FontId::proportional(20.0),
          color: Color32::YELLOW,
          ..Default::default()
        },
      });
    }

    if job.text.len() != final_start {
      job.sections.push(LayoutSection {
        leading_space: 0.0,
        byte_range: final_start..job.text.len(),
        format: TextFormat {
          font_id: FontId::proportional(20.0),
          color: Color32::WHITE,
          ..Default::default()
        },
      });
    }

    (job, cursor)
  }

  fn add_event_to_string(s: &mut String, ev: &Event, needs_space: &mut bool) {
    match ev.event_type {
      EventType::Lyric => {
        if let Some(EventValue::Lyric { text }) = &ev.value {
          if ev.linked_id.is_some() {
            s.push('-');
          } else if *needs_space {
            s.push(' ');
          }
          s.push_str(text.as_str());
          *needs_space = true;
        }
      }
      EventType::LineBreak => {
        s.push('\n');
        *needs_space = false;
      }
      EventType::ParagraphBreak => {
        s.push_str("\n\n");
        *needs_space = false;
      }
      _ => {}
    }
  }

  fn handle_sync(&mut self, app: &KsngApp, track: Option<&Track>) {
    let Some(track) = track else {
      return;
    };

    if self.current_idx >= track.events.len() {
      return;
    }

    // God this logic is so complicated. There *must* be a better way!

    let time = app.playback.borrow().position().max(self.min_time);
    let mut last_event_end = time;

    if !self.events_need_repositioning.is_empty() {
      let mut repos_start = match self.last_idx {
        Some(last_idx) => {
          if self.finished_last_syllable {
            self.event_timings[last_idx].1
          } else {
            self.min_time
          }
        }
        None => Timecode(0),
      };
      let orig_start = self
        .events_need_repositioning
        .iter()
        .map(|idx| self.event_timings[*idx].0)
        .min()
        .unwrap();
      let orig_end = self
        .events_need_repositioning
        .iter()
        .map(|idx| self.event_timings[*idx].1)
        .max()
        .unwrap();
      let orig_length = orig_end - orig_start;
      let mut available_length = time - repos_start;
      let mut current_start = time;
      // If we have a previous event we still need to end, give it half the length.
      if !self.finished_last_syllable {
        available_length = Timecode::from_seconds_f64(available_length.to_seconds_f64() / 2.0)
          .max(Timecode::from_seconds(0.05));
        repos_start += available_length;
        last_event_end = repos_start;
        current_start = repos_start + available_length;
      }

      if orig_length <= available_length {
        // We have enough room to fit the events without changing their duration.
        let mut next_end = current_start;
        for event_idx in &self.events_need_repositioning {
          let len = self.event_timings[*event_idx].1 - self.event_timings[*event_idx].0;
          self.event_timings[*event_idx] = (next_end - len, next_end);
          next_end -= len;
        }
        self.min_time = self.min_time.max(current_start);
      } else {
        // We need to change the event length to fit the available duration.
        let repos_length = Timecode::from_seconds_f64(
          (available_length.to_seconds_f64() / self.events_need_repositioning.len() as f64)
            .min(0.05),
        );

        for event_idx in &self.events_need_repositioning {
          self.event_timings[*event_idx] = (repos_start, repos_start + repos_length);
          repos_start += repos_length;
        }
        self.min_time = self.min_time.max(repos_start);
      }

      self.events_need_repositioning.clear();
    }

    if !self.finished_last_syllable
      && let Some(last_idx) = self.last_idx
    {
      self.event_timings[last_idx] = (self.event_timings[last_idx].0, last_event_end);
    }

    let length = self.event_timings[self.current_idx].1 - self.event_timings[self.current_idx].0;
    let end_time = time + length;

    let time = time.max(self.min_time);
    self.event_timings[self.current_idx] = (time, end_time);
    self.undo_context.push((
      self.current_idx,
      self.last_idx,
      app.playback.borrow().position(),
      self.min_time,
    ));

    self.last_idx = Some(self.current_idx);
    self.finished_last_syllable = false;

    self.current_idx += 1;
    while self.current_idx < track.events.len()
      && track.events[self.current_idx].event_type != EventType::Lyric
    {
      self.events_need_repositioning.push(self.current_idx);
      self.current_idx += 1;
    }

    self.layout_job = None;
    // Make sure we don't place another event too soon after this one, even if
    // that's not the "proper" timing.
    self.min_time = time + Timecode::from_seconds(0.05);
    self.pending_scroll = true;
    self.is_dirty = true;
  }

  fn handle_break(&mut self, app: &KsngApp) {
    let Some(last_idx) = self.last_idx else {
      return;
    };

    if self.finished_last_syllable {
      return;
    }

    let time = app.playback.borrow().position().max(self.min_time);
    self.event_timings[last_idx] = (self.event_timings[last_idx].0, time);
    self.finished_last_syllable = true;

    self.layout_job = None;
    self.pending_scroll = true;
    self.min_time = time + Timecode::from_seconds(0.05);
  }

  fn handle_back(&mut self, app: &KsngApp) {
    let Some((idx, last_idx, pos, min_time)) = self.undo_context.pop() else {
      return;
    };

    self.events_need_repositioning.clear();
    self.min_time = min_time;
    self.last_idx = last_idx;
    self.current_idx = idx;
    app.playback.borrow_mut().seek(pos);
    self.finished_last_syllable = true;
    self.layout_job = None;
    self.pending_scroll = true;
  }

  fn handle_save(&mut self, app: &KsngApp, track: Option<&Track>) {
    if let Some(last_idx) = self.last_idx
      && let Some(track) = track
      && last_idx < track.events.len() - 1
    {
      // There are events we didn't get to - move them to after all synced
      // events so that they don't overlap with what we just synced.
      let mut last_end_pos = if self.finished_last_syllable {
        self.event_timings[last_idx].1
      } else {
        self.min_time
      };

      self.event_timings[last_idx] = (self.event_timings[last_idx].0, last_end_pos);

      let diff = last_end_pos - self.event_timings[last_idx + 1].0;

      for idx in (last_idx + 1)..self.event_timings.len() {
        if self.event_timings[idx].0 >= last_end_pos {
          // This doesn't overlap, stop moving things.
          break;
        }

        let length = self.event_timings[idx].1 - self.event_timings[idx].0;
        self.event_timings[idx] = (
          self.event_timings[idx].0 + diff,
          self.event_timings[idx].0 + diff + length,
        );

        last_end_pos = self.event_timings[idx].1;
      }
    }

    app
      .commands
      .dispatch(SetEventTimingsCommand::new_with_title(
        "Sync timings".to_string(),
        &self.event_ids,
        &self.event_timings,
      ));
    self.is_dirty = false;
  }
}

impl KWindow for SyncWindow {
  fn should_cleanup(&self) -> bool {
    !self.open
  }

  fn process(&mut self, app: &crate::KsngApp, context: &egui::Context) {
    if !self.open {
      return;
    }

    // TODO: handle track change

    let window = egui::Window::new("Sync Lyrics")
      .min_width(200.0)
      .min_height(200.0)
      .show(context, |ui| {
        let project = app.project.borrow();
        let track = project
          .iter()
          .flat_map(|p| p.file.tracks.iter().find(|t| t.id == self.track_id))
          .next();

        if let Some(track) = track
          && !track.events.is_empty()
          && self.event_timings.is_empty()
        {
          self.event_timings = track
            .events
            .iter()
            .map(|t| (t.start_timecode, t.end_timecode))
            .collect();
					self.event_ids = track.events.iter().map(|t| t.id).collect();
        }

        if let Some(track) = track
          && self.current_idx == 0
          && !track.events.is_empty()
          && track.events[self.current_idx].event_type != EventType::Lyric
        {
          self.current_idx += 1;
          while self.current_idx < track.events.len()
            && track.events[self.current_idx].event_type != EventType::Lyric
          {
            self.events_need_repositioning.push(self.current_idx);
            self.current_idx += 1;
          }
        }

        let is_at_end = track.is_none() || self.current_idx >= track.as_ref().unwrap().events.len();

				let mut handle_sync = false;
				let mut handle_break = false;
				let mut handle_back = false;
				let mut handle_save = false;

        if ui.input_mut(|i| {
          i.consume_key(egui::Modifiers::NONE, Key::Z)
            || i.consume_key(egui::Modifiers::NONE, Key::X)
        }) {
          handle_sync = true;
        }

        if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Space)) {
          handle_break = true;
        }

				if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Backspace)) {
					handle_back = true;
				}

        egui::TopBottomPanel::bottom("sync#buttons").show_inside(ui, |ui| {
          ui.add_space(5.0);
          Sides::new().show(
            ui,
            |ui| {
              if ui
                .add_enabled(!is_at_end, Button::new("Sync (Z/X)"))
                .clicked()
              {
                handle_sync = true;
              }

              if ui
                .add_enabled(!self.finished_last_syllable, Button::new("Break (Space)"))
                .clicked()
              {
                handle_break = true;
              }

              if ui
                .add_enabled(self.last_idx.is_some(), Button::new("Back (Backspace)"))
                .clicked()
              {
                handle_back = true;
              }
            },
            |ui| {
              if ui.button("Cancel").clicked() {
								if self.is_dirty {
									// We have changes.
									app.modals.add(ConfirmModal::new(
										"Discard sync changes?".to_string(),
										"You have made changes to event synchronization. If you cancel, these changes will be discarded. Are you sure?".to_string(),
										KsngEvent::CloseWindow(self.unique_value)
									));
								} else {
									self.open = false;
								}
							}
              if ui.button("Save").clicked() {
								handle_save = true;
							}
            },
          );
        });

				if handle_sync {
					self.handle_sync(app, track);
				}

				if handle_break {
					self.handle_break(app);
				}

				if handle_back {
					self.handle_back(app);
				}

				if handle_save {
					self.handle_save(app, track);
				}

        if self.layout_job.is_none() {
          let (job, cursor) = Self::layout_lyrics(track, self.current_idx);
          self.layout_job = Some(job);
          self.layout_cursor = cursor;
        }

        egui::CentralPanel::default().show_inside(ui, |ui| {
          egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
            let mut layout = self.layout_job.as_ref().unwrap().clone();
            let mut text = layout.text.clone();
            let mut layouter = |ui: &egui::Ui, _buf: &dyn egui::TextBuffer, wrap_width: f32| {
              layout.wrap.max_width = wrap_width;
              ui.fonts(|f| f.layout_job(layout.clone()))
            };

            let text_edit_id = Id::new("sync#lyrics_text");

            let response = egui::TextEdit::multiline(&mut text)
              .id(text_edit_id)
              .frame(false)
              .interactive(false)
              .desired_width(ui.available_width() - 20.0)
              .font(FontId::proportional(20.0))
              .layouter(&mut layouter)
              .show(ui);

            if self.pending_scroll {
              // Scroll to current position every time sync is clicked
              let rect = response.galley.pos_from_cursor(self.layout_cursor);
              let rect = rect.translate(response.galley_pos.to_vec2());
              ui.scroll_to_rect(rect, Some(egui::Align::Center));
              self.pending_scroll = false;
            }
          });
        });
      });

    if let Some(window) = window
      && self.should_request_focus
    {
      window.response.request_focus();
      self.should_request_focus = false;
    }
  }

  fn request_focus(&mut self) {
    self.should_request_focus = true;
  }

  fn unique_value(&self) -> Option<u64> {
    Some(self.unique_value)
  }
}
