use egui::{
  Color32, FontId, TextFormat, Ui,
  text::{LayoutJob, LayoutSection},
};
use klib::objects::{
  event::{Event, EventType, EventValue},
  track::Track,
};
use uuid::Uuid;

use crate::windows::KWindow;

pub struct SyncWindow {
  open: bool,
  should_request_focus: bool,
  track_id: Uuid,
  layout_job: Option<LayoutJob>,
  current_idx: usize,
}

impl SyncWindow {
  pub fn new(track_id: Uuid) -> SyncWindow {
    SyncWindow {
      open: true,
      should_request_focus: false,
      track_id,
      current_idx: 0,
      layout_job: None,
    }
  }

  fn layout_lyrics(track: Option<&Track>, current_idx: usize) -> LayoutJob {
    let Some(track) = track else {
      return LayoutJob::default();
    };

    assert!(current_idx < track.events.len());

    let mut s = String::new();
    let mut needs_space = false;
    for idx in 0..current_idx {
      if let Some(ev) = track.events.get(idx) {
        Self::add_event_to_string(&mut s, ev, &mut needs_space);
      }
    }

    let before_len = s.len();
    if let Some(ev) = track.events.get(current_idx) {
      Self::add_event_to_string(&mut s, ev, &mut needs_space);
    }

    let current_len = s.len() - before_len;
    let final_start = s.len();

    for idx in (current_idx + 1)..track.events.len() {
      if let Some(ev) = track.events.get(idx) {
        Self::add_event_to_string(&mut s, ev, &mut needs_space);
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

    job
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
}

impl KWindow for SyncWindow {
  fn should_cleanup(&self) -> bool {
    !self.open
  }

  fn process(&mut self, app: &crate::KsngApp, context: &egui::Context) {
    if !self.open {
      return;
    }

    let window = egui::Window::new("Sync Lyrics")
      .min_width(200.0)
      .min_height(200.0)
      .show(context, |ui| {
        let project = app.project.borrow();
        let track = project
          .iter()
          .flat_map(|p| p.file.tracks.iter().find(|t| t.id == self.track_id))
          .next();

        if self.layout_job.is_none() {
          self.layout_job = Some(Self::layout_lyrics(track, self.current_idx));
        }

        egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
          let mut layout = self.layout_job.as_ref().unwrap().clone();
          let mut text = layout.text.clone();
          let mut layouter = |ui: &egui::Ui, _buf: &dyn egui::TextBuffer, wrap_width: f32| {
            layout.wrap.max_width = wrap_width;
            ui.fonts(|f| f.layout_job(layout.clone()))
          };

          egui::TextEdit::multiline(&mut text)
            .frame(false)
            .interactive(false)
            .desired_width(ui.available_width() - 20.0)
            .font(FontId::proportional(20.0))
            .layouter(&mut layouter)
            .show(ui);
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
    todo!()
  }
}
