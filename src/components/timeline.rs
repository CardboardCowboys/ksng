use std::collections::{hash_map::Entry, HashMap};

use egui::{
  scroll_area::ScrollSource, Align2, CentralPanel, Color32, Context, FontId, Frame, Id,
  ImageButton, ImageSource, Margin, PointerButton, Pos2, Rect, ScrollArea, Sense, SidePanel, Sides,
  Stroke, StrokeKind, TextureOptions, Ui, UiBuilder, Vec2,
};
use klib::{
  objects::{
    event::{Event, EventType},
    track::{EventList, TrackValue},
  },
  timecode::Timecode,
};
use uuid::Uuid;

use crate::{
  commands::track::MuteTrackCommand,
  style::{
    colors::{self, color_for_track_type},
    icons,
  },
  util::ui::KsngUiExt,
  windows::track_config::TrackConfigWindow,
  KsngApp,
};

pub const TRACK_HEIGHT: f32 = 50.0;
const TRACK_INNER_PADDING: i8 = 2;
const TRACK_HEADER_WIDTH: f32 = 200.0;
const PIXELS_PER_SECOND: f32 = 40.0;
const MIN_X_ZOOM: f32 = 0.01;
const MAX_X_ZOOM: f32 = 20.0;
const MIN_Y_ZOOM: f32 = 0.5;
const MAX_Y_ZOOM: f32 = 20.0;

const SCRUB_AREA: f32 = 8.0;

#[derive(PartialEq)]
enum TimelineUiState {
  /// Nothing special is happening.
  Idle,
  /// We don't know what we're doing yet.
  Pending,
  /// We're currently box selecting.
  Multiselect,
  /// We're currently scrubbing the timeline.
  Scrubbing,
  /// We're currently dragging an event.
  Dragging,
}

pub struct Timeline {
  zoom: Vec2,
  horiz_scroll_offset: f32,
  event_text: HashMap<Uuid, String>,
  state: TimelineUiState,
}

impl Default for Timeline {
  fn default() -> Self {
    Self {
      zoom: Vec2::new(1.0, 1.0),
      horiz_scroll_offset: 0.0,
      event_text: Default::default(),
      state: TimelineUiState::Idle,
    }
  }
}

impl Timeline {
  pub fn update(&mut self, app: &KsngApp, _ctx: &Context, ui: &mut Ui) {
    let zoom_delta = ui.input_mut(|input| {
      if input.modifiers.alt {
        Vec2::new(0.0, input.zoom_delta() - 1.0)
      } else {
        Vec2::new(input.zoom_delta() - 1.0, 0.0)
      }
    });

    ui.input_mut(|input| {
      if zoom_delta.length_sq() > 0.0 {
        input.smooth_scroll_delta = Vec2::ZERO;
      } else {
        input.smooth_scroll_delta = if input.modifiers.alt {
          input.smooth_scroll_delta
        } else {
          Vec2::new(input.smooth_scroll_delta.y, input.smooth_scroll_delta.x)
        }
      }
    });

    let prev_zoom = self.zoom;

    self.zoom = Vec2::new(
      (self.zoom.x + zoom_delta.x).clamp(MIN_X_ZOOM, MAX_X_ZOOM),
      (self.zoom.y + zoom_delta.y).clamp(MIN_Y_ZOOM, MAX_Y_ZOOM),
    );

    let mouse_pos = ui.input(|input| input.pointer.latest_pos());
    let is_shift = ui.input(|input| input.modifiers.shift);

    let track_height = TRACK_HEIGHT * self.zoom.y;
    let pixels_per_second = PIXELS_PER_SECOND * self.zoom.x;

    ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
      let project = app.project.borrow();
      if project.is_none() {
        return;
      }
      let project = project.as_ref().unwrap();

      ui.visuals_mut().clip_rect_margin = 0.0;
      ui.spacing_mut().item_spacing = Vec2::ZERO;
      ui.spacing_mut().indent = 0.0;

      SidePanel::left(Id::new("timeline#headers"))
        .frame(Frame::side_top_panel(ui.style()))
        .show_inside(ui, |ui| {
          for track in &project.file.tracks {
            let mut frame = Frame::new()
              .corner_radius(0)
              .outer_margin(Margin::symmetric(0, TRACK_INNER_PADDING))
              .fill(color_for_track_type(track.track_type));

            if app.selection.is_track_selected(track.id) {
              frame = frame.stroke(Stroke::new(1.0, Color32::WHITE));
            } else {
              frame = frame.stroke(Stroke::new(
                1.0,
                Color32::from_rgba_premultiplied(0, 0, 0, 0),
              ));
            }

            let mut buttons_clicked = false;
            let res = frame.show(ui, |ui| {
              ui.set_height(track_height);
              ui.set_width(TRACK_HEADER_WIDTH);
              ui.style_mut().visuals.override_text_color = Some(Color32::WHITE);
              let rect = ui.max_rect();
              let contents_rect = Rect {
                min: Pos2::new(rect.min.x + 2.0, rect.min.y + 2.0),
                max: Pos2::new(rect.max.x - 2.0, rect.max.y - 2.0),
              };
              let mut child_ui = ui.new_child(UiBuilder::new().max_rect(contents_rect));
              Sides::new().show(
                &mut child_ui,
                |ui| {
                  ui.inert_heading(format!("{:?}", track.track_type));
                },
                |ui| {
                  let settings_button = ImageButton::new(icons::GEAR);
                  if ui
                    .add_sized(Vec2::new(20.0, 20.0), settings_button)
                    .clicked()
                  {
                    app.windows.add(TrackConfigWindow::new(track));
                    buttons_clicked = true;
                  }

                  ui.add_space(2.0);

                  if let Some(TrackValue::Audio(audio)) = &track.track_value {
                    let mut mute_button = ImageButton::new(if audio.muted {
                      icons::VOLUME_OFF
                    } else {
                      icons::VOLUME
                    });

                    if audio.muted {
                      mute_button = mute_button.tint(Color32::RED);
                    }

                    if ui.add_sized(Vec2::new(20.0, 20.0), mute_button).clicked() {
                      app.commands.dispatch(MuteTrackCommand::new(track));
                      buttons_clicked = true;
                    }
                  }
                },
              );
            });

            let header_clicked = !buttons_clicked
              && ui.input(|input| {
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

      CentralPanel::default()
        .frame(Frame::side_top_panel(ui.style()))
        .show_inside(ui, |ui| {
          let mut scroll_source = ScrollSource::ALL;
          if self.state != TimelineUiState::Idle {
            scroll_source.drag = false;
          }

          let res: egui::scroll_area::ScrollAreaOutput<()> = ScrollArea::horizontal()
            .auto_shrink(false)
            .animated(false)
            .scroll_source(scroll_source)
            .horizontal_scroll_offset(self.horiz_scroll_offset)
            .show_viewport(ui, |ui, viewport_rect| {
              // Set playhead rect and check for interactions before checking event
              // interactions.
              let playhead_rect = ui.max_rect();
              let playhead_pos = app.playback.borrow().position().to_seconds() * pixels_per_second;
              let playhead_pos2 =
                Pos2::new(playhead_rect.min.x + playhead_pos, playhead_rect.min.y);

              // Update playhead state.
              let scrub_rect = Rect {
                min: Pos2::new(playhead_pos2.x - SCRUB_AREA / 2.0, playhead_pos2.y),
                max: Pos2::new(playhead_pos2.x + SCRUB_AREA / 2.0, playhead_rect.max.y),
              };

              if let Some(mouse_pos) = mouse_pos {
                if self.state == TimelineUiState::Idle && scrub_rect.contains(mouse_pos) {
                  ui.input(|input| {
                    if input.pointer.primary_down() {
                      self.state = TimelineUiState::Scrubbing;
                    }
                  });
                } else if self.state == TimelineUiState::Scrubbing {
                  ui.input(|input| {
                    if !input.pointer.primary_down() {
                      self.state = TimelineUiState::Idle;
                    }
                  });
                }
              } else if self.state != TimelineUiState::Idle {
                self.state = TimelineUiState::Idle;
              }

              for track in &project.file.tracks {
                let len = track
                  .events
                  .iter()
                  .max_by_key(|e| e.end_timecode)
                  .map(|ev| ev.end_timecode)
                  .unwrap_or_default();
                let width = len.to_seconds() * pixels_per_second;
                let (_id, rect) = ui.allocate_space(Vec2::new(
                  width,
                  track_height + TRACK_INNER_PADDING as f32 * 3.0,
                ));
                let visible_len = Timecode::from_seconds(viewport_rect.width() / pixels_per_second);
                let visible_start = Timecode::from_seconds(viewport_rect.min.x / pixels_per_second);

                for ev in track
                  .events
                  .events_in_range((visible_start, visible_start + visible_len))
                {
                  let start_x = ev.start_timecode.to_seconds() * pixels_per_second + rect.min.x;
                  let end_x = ev.end_timecode.to_seconds() * pixels_per_second + rect.min.x;
                  let width = (end_x - start_x).max(1.0);
                  let rect = Rect {
                    min: Pos2::new(start_x, rect.min.y + TRACK_INNER_PADDING as f32),
                    max: Pos2::new(start_x + width, rect.max.y - TRACK_INNER_PADDING as f32),
                  };

                  let response = ui.allocate_rect(rect, Sense::click_and_drag());

                  if response.clicked() && self.state == TimelineUiState::Idle {
                    app.selection.select_event(ev.id, !is_shift);
                  }

                  let color = colors::color_for_event_type(ev.event_type);
                  let stroke_color = if app.selection.is_event_selected(ev.id) {
                    colors::SELECTED_COLOR
                  } else {
                    colors::darkened_color_for_event_type(ev.event_type)
                  };

                  ui.painter().rect_filled(rect, 0, color);

                  if ev.event_type == EventType::AudioClip {
                    if let Some(image) = app.waveforms.borrow().get_image(ev) {
                      let image = egui::Image::new(ImageSource::Uri(image.as_ref().into()))
                        .texture_options(TextureOptions::NEAREST)
                        .tint(Color32::from_black_alpha(127));
                      image.paint_at(ui, rect);
                    }
                  }

                  ui.painter().rect_stroke(
                    rect,
                    0,
                    Stroke::new(1.0, stroke_color),
                    StrokeKind::Inside,
                  );

                  if width > 5.0 {
                    self.draw_event_text(ui, ev, &rect);
                  }
                }
              }

              // Draw playhead
              ui.painter().rect_filled(
                Rect {
                  min: Pos2::new(playhead_pos2.x - 0.5, playhead_pos2.y),
                  max: Pos2::new(playhead_pos2.x + 0.5, playhead_rect.max.y),
                },
                0,
                colors::PLAYHEAD_COLOR,
              );

              let mut mesh = egui::Mesh::default();
              mesh.colored_vertex(
                Pos2::new(playhead_pos2.x - 5.0, playhead_pos2.y),
                colors::PLAYHEAD_TOP_COLOR,
              );
              mesh.colored_vertex(
                Pos2::new(playhead_pos2.x + 5.0, playhead_pos2.y),
                colors::PLAYHEAD_TOP_COLOR,
              );
              mesh.colored_vertex(
                Pos2::new(playhead_pos2.x, playhead_pos2.y + 10.0),
                colors::PLAYHEAD_TOP_COLOR,
              );
              mesh.add_triangle(0, 1, 2);
              ui.painter().add(egui::Shape::mesh(mesh));
            });

          self.horiz_scroll_offset = res.state.offset.x;

          // Try repositioning the scroll area to put the cursor where it was before the
          // zoom.
          if let Some(cursor_pos) = ui.input(|input| input.pointer.latest_pos()) {
            let content_pos_x = cursor_pos.x - res.inner_rect.min.x + self.horiz_scroll_offset;
            if (self.zoom.x - prev_zoom.x).abs() > 0.0 && res.inner_rect.contains(cursor_pos) {
              let prev_pps = PIXELS_PER_SECOND * prev_zoom.x;
              let prev_pos_s = content_pos_x / prev_pps;
              let new_pos_s = content_pos_x / pixels_per_second;
              let diff_pixels = (new_pos_s - prev_pos_s) * pixels_per_second;
              self.horiz_scroll_offset -= diff_pixels;
            } else if self.state == TimelineUiState::Scrubbing {
              let cursor_pos_s = content_pos_x / pixels_per_second;
              app
                .playback
                .borrow_mut()
                .seek(Timecode::from_seconds(cursor_pos_s));
            }
          }
        });
    });
  }

  fn draw_event_text(&mut self, ui: &mut Ui, event: &Event, rect: &Rect) {
    match self.event_text.entry(event.id) {
      Entry::Occupied(entry) => Self::draw_event_text_impl(ui, rect, entry.get().as_ref()),
      Entry::Vacant(entry) => Self::draw_event_text_impl(
        ui,
        rect,
        entry.insert(Self::create_event_text(event)).as_ref(),
      ),
    }
  }

  fn draw_event_text_impl(ui: &mut Ui, rect: &Rect, text: &str) {
    let font = FontId::proportional(12.0);
    let inner_rect = rect.expand2(Vec2::new(2.0, 2.0));
    ui.painter().with_clip_rect(inner_rect).text(
      Pos2::new(rect.min.x + 2.0, rect.min.y + 2.0),
      Align2::LEFT_TOP,
      text,
      font,
      Color32::WHITE,
    );
  }

  fn create_event_text(event: &Event) -> String {
    let name = match event.event_type {
      EventType::Lyric => "Lyric",
      EventType::LineBreak => "LineBreak",
      EventType::ParagraphBreak => "ParagraphBreak",
      EventType::AudioClip => "AudioClip",
      EventType::Image => "Image",
    }
    .to_owned();
    if let Some(s) = event.description() {
      if event.event_type == EventType::Lyric {
        format!("{name}\n'{s}'")
      } else {
        format!("{name}\n{s}")
      }
    } else {
      name
    }
  }
}
