use egui::{
  CentralPanel, Color32, Context, Frame, Id, Margin, PointerButton, Pos2, Rect, ScrollArea,
  SidePanel, Stroke, StrokeKind, Ui, Vec2,
};
use klib::{objects::track::EventList, timecode::Timecode};

use crate::{
  style::colors::{self, color_for_track_type},
  KsngApp,
};

const TRACK_HEIGHT: f32 = 50.0;
const TRACK_INNER_PADDING: i8 = 2;
const TRACK_HEADER_WIDTH: f32 = 200.0;
const PIXELS_PER_SECOND: f32 = 40.0;

pub struct Timeline {
  zoom: Vec2,
}

impl Default for Timeline {
  fn default() -> Self {
    Self {
      zoom: Vec2::new(1.0, 1.0),
    }
  }
}

impl Timeline {
  pub fn update(&mut self, app: &KsngApp, ctx: &Context, ui: &mut Ui) {
    ui.input_mut(|input| {
      input.smooth_scroll_delta = if input.modifiers.alt {
        input.smooth_scroll_delta
      } else {
        Vec2::new(input.smooth_scroll_delta.y, input.smooth_scroll_delta.x)
      }
    });

    ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
      let project = app.project.borrow();
      if project.is_none() {
        return;
      }
      let project = project.as_ref().unwrap();

      SidePanel::left(Id::new("timeline#headers")).show_inside(ui, |ui| {
        for track in &project.file.tracks {
          let mut frame = Frame::new()
            .corner_radius(0)
            .outer_margin(Margin::symmetric(0, TRACK_INNER_PADDING))
            .inner_margin(5)
            .fill(color_for_track_type(track.track_type));

          if app.selection.is_track_selected(track.id) {
            frame = frame.stroke(Stroke::new(1.0, Color32::WHITE));
          } else {
            frame = frame.stroke(Stroke::new(
              1.0,
              Color32::from_rgba_premultiplied(0, 0, 0, 0),
            ));
          }

          let res = frame.show(ui, |ui| {
            ui.set_height(TRACK_HEIGHT);
            ui.set_width(TRACK_HEADER_WIDTH);
            ui.style_mut().visuals.override_text_color = Some(Color32::WHITE);
            ui.heading(format!("{:?}", track.track_type));
          });

          let header_clicked = ui.input(|input| {
            input.pointer.button_clicked(PointerButton::Primary)
              && res
                .response
                .rect
                .contains(input.pointer.interact_pos().unwrap())
          });

          if header_clicked {
            let single = !ui.input(|i| i.modifiers.shift);
            app.selection.select_track(track.id, single);
          }
        }
      });

      CentralPanel::default().show_inside(ui, |ui| {
        ScrollArea::horizontal()
          .auto_shrink(false)
          .show_viewport(ui, |ui, viewport_rect| {
            for track in &project.file.tracks {
              let frame = Frame::new().fill(Color32::WHITE).show(ui, |ui| {
                ui.set_height(TRACK_HEIGHT + TRACK_INNER_PADDING as f32 * 2.0 + 10.0);
                let len = track
                  .events
                  .iter()
                  .max_by_key(|e| e.end_timecode)
                  .map(|ev| ev.end_timecode)
                  .unwrap_or_default();
                let width = len.to_seconds() * PIXELS_PER_SECOND;
                ui.set_width(width);
              });

              let rect = frame.response.rect;
              let visible_len = Timecode::from_seconds(viewport_rect.width() / PIXELS_PER_SECOND);
              let visible_start = Timecode::from_seconds(viewport_rect.min.x / PIXELS_PER_SECOND);

              for ev in track
                .events
                .events_in_range((visible_start, visible_start + visible_len))
              {
                let start_x = ev.start_timecode.to_seconds() * PIXELS_PER_SECOND + rect.min.x;
                let end_x = ev.end_timecode.to_seconds() * PIXELS_PER_SECOND + rect.min.x;
                let width = (end_x - start_x).max(1.0);
                let rect = Rect {
                  min: Pos2::new(start_x, rect.min.y),
                  max: Pos2::new(start_x + width, rect.max.y),
                };

                let color = colors::color_for_event_type(ev.event_type);
                let stroke_color = colors::darkened_color_for_event_type(ev.event_type);

                ui.painter().rect_filled(rect, 0, color);
                ui.painter().rect_stroke(
                  rect,
                  0,
                  Stroke::new(1.0, stroke_color),
                  StrokeKind::Inside,
                );
              }
            }
          });
      });
    });
  }
}
