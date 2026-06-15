use std::collections::{HashMap, HashSet, hash_map::Entry};

use egui::{
  Align2, CentralPanel, Color32, Context, CursorIcon, FontId, Frame, Id, ImageButton, ImageSource,
  Margin, PointerButton, Pos2, Rect, ScrollArea, Sense, SidePanel, Sides, Stroke, StrokeKind,
  TextureOptions, Ui, UiBuilder, Vec2, scroll_area::ScrollSource,
};
use klib::{
  objects::{
    event::{Event, EventType},
    track::{EventList, TrackType, TrackValue},
  },
  timecode::Timecode,
};
use uuid::Uuid;

use crate::{
  KsngApp,
  commands::{event::SetEventTimingsCommand, track::MuteTrackCommand},
  project::Project,
  style::{
    colors::{self, color_for_track_type},
    icons,
  },
  util::ui::KsngUiExt,
  windows::{sync::SyncWindow, track_config::TrackConfigWindow},
};

pub const TRACK_HEIGHT: f32 = 50.0;
const TRACK_INNER_PADDING: i8 = 2;
const TRACK_HEADER_WIDTH: f32 = 200.0;
const PIXELS_PER_SECOND: f32 = 40.0;
const MIN_X_ZOOM: f32 = 0.01;
const MAX_X_ZOOM: f32 = 20.0;
const MIN_Y_ZOOM: f32 = 0.5;
const MAX_Y_ZOOM: f32 = 20.0;

const EVENT_HANDLE_WIDTH: f32 = 5.0;
const SCRUB_AREA: f32 = 8.0;
const SNAP_WIDTH: f32 = 10.0;

const MIN_EVENT_SIZE: Timecode = Timecode(100);

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

#[derive(PartialEq)]
enum DragType {
  Start,
  End,
  MoveEvent,
}

struct DragState {
  events: Vec<Uuid>,
  timings: Vec<(Timecode, Timecode)>,
  orig_timings: Vec<(Timecode, Timecode)>,
  drag_type: DragType,
}

impl DragState {
  pub fn new(project: &Project, drag_type: DragType, events: Vec<Uuid>) -> DragState {
    // dedup events
    let events: HashSet<Uuid> = HashSet::from_iter(events);
    let events = Vec::from_iter(events);

    let mut events_timings = Vec::with_capacity(events.len());
    for ev in &events {
      for track in &project.file.tracks {
        if let Some(ev) = track.events.find_id(*ev) {
          events_timings.push((ev.id, ev.start_timecode, ev.end_timecode));
          break;
        }
      }
    }

    events_timings.sort_by_key(|(_, start, _)| *start);

    let events = Vec::from_iter(events_timings.iter().map(|(e, _, _)| *e));
    let timings = Vec::from_iter(
      events_timings
        .into_iter()
        .map(|(_, start, end)| (start, end)),
    );

    assert!(timings.len() == events.len());
    DragState {
      events,
      orig_timings: timings.clone(),
      timings,
      drag_type,
    }
  }
}

pub struct Timeline {
  zoom: Vec2,
  horiz_scroll_offset: f32,
  event_text: HashMap<Uuid, String>,
  state: TimelineUiState,
  drag_state: Option<DragState>,
  is_dragging: bool,
  drag_start_s: Timecode,
  mouse_down_last_pos: Pos2,
}

impl Default for Timeline {
  fn default() -> Self {
    Self {
      zoom: Vec2::new(1.0, 1.0),
      horiz_scroll_offset: 0.0,
      event_text: Default::default(),
      state: TimelineUiState::Idle,
      drag_state: None,
      drag_start_s: Timecode::MIN,
      mouse_down_last_pos: Pos2::ZERO,
      is_dragging: false,
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

    let mut track_clicked = false;

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
              frame = frame.stroke(Stroke::new(1.0_f32, Color32::WHITE));
            } else {
              frame = frame.stroke(Stroke::new(
                1.0_f32,
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

                  if track.track_type == TrackType::Lyrics
                    && ui
                      .add_sized(Vec2::new(20.0, 20.0), ImageButton::new(icons::SYNC))
                      .clicked()
                  {
                    app.windows.add(SyncWindow::new(track.id));
                    buttons_clicked = true;
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
              track_clicked = true;
            }
          }
        });

      CentralPanel::default()
        .frame(Frame::side_top_panel(ui.style()))
        .show_inside(ui, |ui| {
          let mut scroll_source = ScrollSource::ALL;
          scroll_source.drag = false;

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
                } else if self.state == TimelineUiState::Idle {
                  ui.input(|input| {
                    if input.pointer.primary_down() && ui.max_rect().contains(mouse_pos) {
                      self.state = TimelineUiState::Pending;
                      self.mouse_down_last_pos = mouse_pos;
                    }
                  });
                }
              } else if self.state != TimelineUiState::Idle {
                self.state = TimelineUiState::Idle;
              }

              if !self.is_dragging {
                self.drag_state = None;
              }

              let multiselect_box = if self.state == TimelineUiState::Multiselect
                && let Some(mouse_pos) = mouse_pos
              {
                Rect::from_points(&[mouse_pos, self.mouse_down_last_pos])
              } else {
                Rect::ZERO
              };
              let mut multiselect_events = Vec::new();
              let mut pointer_over = None;
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
                  let (mut start, mut end) = (ev.start_timecode, ev.end_timecode);
                  if let Some(drag_state) = self.drag_state.as_ref()
                    && let Some(idx) = drag_state.events.iter().position(|e| *e == ev.id)
                  {
                    start = drag_state.timings[idx].0;
                    end = drag_state.timings[idx].1;
                  }

                  let start_x = start.to_seconds() * pixels_per_second + rect.min.x;
                  let end_x = end.to_seconds() * pixels_per_second + rect.min.x;
                  let width = (end_x - start_x).max(1.0);
                  let rect = Rect {
                    min: Pos2::new(start_x, rect.min.y + TRACK_INNER_PADDING as f32),
                    max: Pos2::new(start_x + width, rect.max.y - TRACK_INNER_PADDING as f32),
                  };

                  if rect.intersects(multiselect_box) {
                    multiselect_events.push(ev.id);
                  }

                  let response = ui.allocate_rect(rect, Sense::click_and_drag());

                  if response.contains_pointer() {
                    pointer_over = Some(ev.id);
                  }

                  if self.drag_state.is_none() {
                    let is_touching_start = if let Some(mouse_pos) = mouse_pos
                      && mouse_pos.y >= rect.min.y
                      && mouse_pos.y < rect.max.y
                      && (start_x - mouse_pos.x).abs() < EVENT_HANDLE_WIDTH
                    {
                      true
                    } else {
                      false
                    };
                    let is_touching_end = if let Some(mouse_pos) = mouse_pos
                      && mouse_pos.y >= rect.min.y
                      && mouse_pos.y < rect.max.y
                      && (end_x - mouse_pos.x).abs() < EVENT_HANDLE_WIDTH
                    {
                      true
                    } else {
                      false
                    };

                    let drag_type = if is_touching_start {
                      ui.ctx().set_cursor_icon(CursorIcon::ResizeEast);
                      Some(DragType::Start)
                    } else if is_touching_end {
                      ui.ctx().set_cursor_icon(CursorIcon::ResizeWest);
                      Some(DragType::End)
                    } else if response.contains_pointer() {
                      ui.ctx().set_cursor_icon(CursorIcon::Grab);
                      Some(DragType::MoveEvent)
                    } else {
                      None
                    };

                    if let Some(drag_type) = drag_type {
                      let mut events = vec![ev.id];
                      if drag_type == DragType::MoveEvent
                        && app.selection.selected_events().len() > 1
                      {
                        for ev in app.selection.selected_events() {
                          events.push(ev);
                        }
                      }

                      self.drag_state = Some(DragState::new(project, drag_type, events));
                    }
                  }

                  /*if response.clicked() && self.state == TimelineUiState::Idle {
                    app.selection.select_event(ev.id, !is_shift);
                  }*/

                  let color = colors::color_for_event_type(ev.event_type);
                  let stroke_color = if app.selection.is_event_selected(ev.id) {
                    colors::SELECTED_COLOR
                  } else {
                    colors::darkened_color_for_event_type(ev.event_type)
                  };

                  ui.painter().rect_filled(rect, 0, color);

                  if ev.event_type == EventType::AudioClip
                    && let Some(image) = app.waveforms.borrow().get_image(ev)
                  {
                    let image = egui::Image::new(ImageSource::Uri(image.as_ref().into()))
                      .texture_options(TextureOptions::NEAREST)
                      .tint(Color32::from_black_alpha(127));
                    image.paint_at(ui, rect);
                  }

                  ui.painter().rect_stroke(
                    rect,
                    0,
                    Stroke::new(1.0_f32, stroke_color),
                    StrokeKind::Inside,
                  );

                  if width > 5.0 {
                    self.draw_event_text(ui, ev, &rect);
                  }
                }
              }

              // handle mouse up
              if let Some(mouse_pos) = mouse_pos {
                ui.input(|input| {
                  if !input.pointer.primary_released() {
                    return;
                  }

                  if self.state == TimelineUiState::Pending {
                    let dist = mouse_pos.distance(self.mouse_down_last_pos);
                    if dist < 1.5 {
                      if let Some(over_id) = pointer_over {
                        // select event
                        app.selection.select_event(over_id, !is_shift);
                        self.state = TimelineUiState::Idle;
                      } else {
                        app.selection.clear_events();
                        if ui.max_rect().contains(mouse_pos) {
                          // scrub to pos
                          self.state = TimelineUiState::Scrubbing;
                        }
                      }
                    } else {
                      self.state = TimelineUiState::Idle;
                    }
                  } else if self.state == TimelineUiState::Multiselect {
                    app.selection.clear_events();
                    for ev in multiselect_events {
                      app.selection.select_event(ev, false);
                    }
                    self.state = TimelineUiState::Idle;
                  } else if self.state == TimelineUiState::Dragging {
                    if let Some(drag_state) = &self.drag_state {
                      app.commands.dispatch(SetEventTimingsCommand::new(
                        &drag_state.events,
                        &drag_state.timings,
                      ));
                    }
                    self.is_dragging = false;
                    self.drag_state = None;
                    self.state = TimelineUiState::Idle;
                  }
                })
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

              // draw box select
              if self.state == TimelineUiState::Multiselect
                && let Some(mouse_pos) = mouse_pos
              {
                ui.painter().rect_stroke(
                  Rect::from_points(&[self.mouse_down_last_pos, mouse_pos]),
                  0,
                  Stroke::new(2.0_f32, colors::SELECTED_COLOR),
                  StrokeKind::Middle,
                );
              }
            });

          self.horiz_scroll_offset = res.state.offset.x;

          if self.state == TimelineUiState::Pending
            && let Some(mouse_pos) = mouse_pos
            && ui.max_rect().contains(mouse_pos)
          {
            let dist = mouse_pos.distance(self.mouse_down_last_pos);
            if dist > 1.5 {
              self.state = if self.drag_state.is_some() {
                self.is_dragging = true;
                self.drag_start_s = Timecode::from_seconds(
                  (mouse_pos.x - res.inner_rect.min.x + self.horiz_scroll_offset)
                    / pixels_per_second,
                );
                TimelineUiState::Dragging
              } else {
                TimelineUiState::Multiselect
              };
            }
          }

          // Try repositioning the scroll area to put the cursor where it was before the
          // zoom.
          if let Some(cursor_pos) = ui.input(|input| input.pointer.latest_pos()) {
            let content_pos_x = cursor_pos.x - res.inner_rect.min.x + self.horiz_scroll_offset;
            let cursor_pos_s = content_pos_x / pixels_per_second;
            if (self.zoom.x - prev_zoom.x).abs() > 0.0 && res.inner_rect.contains(cursor_pos) {
              let prev_pps = PIXELS_PER_SECOND * prev_zoom.x;
              let prev_pos_s = content_pos_x / prev_pps;
              let new_pos_s = content_pos_x / pixels_per_second;
              let diff_pixels = (new_pos_s - prev_pos_s) * pixels_per_second;
              self.horiz_scroll_offset -= diff_pixels;
            } else if self.state == TimelineUiState::Scrubbing && !track_clicked {
              app
                .playback
                .borrow_mut()
                .seek(Timecode::from_seconds(cursor_pos_s));
            }

            if self.is_dragging {
              self.update_drag(
                project,
                pixels_per_second,
                Timecode::from_seconds(cursor_pos_s),
                app.playback.borrow().position(),
              );
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

  fn update_drag(
    &mut self,
    project: &Project,
    pixels_per_second: f32,
    mouse_time: Timecode,
    playback_time: Timecode,
  ) {
    let mut max_move_left = u32::MAX;
    let mut max_move_right = u32::MAX;

    let Some(drag_state) = self.drag_state.as_mut() else {
      return;
    };

    if drag_state.events.is_empty() {
      return;
    }

    let event_ids: HashSet<Uuid> = HashSet::from_iter(drag_state.events.clone());

    for (i, ev) in drag_state.events.iter().enumerate() {
      let (min, max) = Self::get_event_track_bounds(project, &event_ids, *ev);
      let (start, end) = drag_state.timings[i];
      let max_move_to_min = start - min;
      let max_move_to_max = max - end;
      max_move_left = max_move_left.min(max_move_to_min.0);
      max_move_right = max_move_right.min(max_move_to_max.0);
    }

    match drag_state.drag_type {
      DragType::Start => {
        let (start, end) = drag_state.timings[0];
        let drag_pos = Self::get_drag_pos(
          pixels_per_second,
          mouse_time,
          start - Timecode(max_move_left),
          end - MIN_EVENT_SIZE,
          &[
            start - Timecode(max_move_left),
            end - MIN_EVENT_SIZE,
            drag_state.orig_timings[0].0,
            playback_time,
          ],
        );
        drag_state.timings[0] = (drag_pos, end);
      }
      DragType::End => {
        let (start, end) = drag_state.timings[0];
        let drag_pos = Self::get_drag_pos(
          pixels_per_second,
          mouse_time,
          start + MIN_EVENT_SIZE,
          end + Timecode(max_move_right),
          &[
            start + MIN_EVENT_SIZE,
            end + Timecode(max_move_right),
            drag_state.orig_timings[0].1,
            playback_time,
          ],
        );
        drag_state.timings[0] = (start, drag_pos);
      }
      DragType::MoveEvent => {
        let len = drag_state
          .orig_timings
          .iter()
          .map(|t| t.1)
          .max()
          .unwrap_or(Timecode::MAX)
          - drag_state
            .orig_timings
            .iter()
            .map(|t| t.0)
            .min()
            .unwrap_or(Timecode::MIN);
        let time_offset = mouse_time.to_seconds() - self.drag_start_s.to_seconds();
        let mouse_time =
          Timecode::from_seconds(drag_state.orig_timings[0].0.to_seconds() + time_offset);
        let (start, end) = (
          drag_state.timings[0].0,
          drag_state
            .timings
            .last()
            .map(|t| t.1)
            .unwrap_or(Timecode::MAX),
        );
        let drag_pos = Self::get_drag_pos(
          pixels_per_second,
          mouse_time,
          start - Timecode(max_move_left),
          end - len + Timecode(max_move_right),
          &[
            start - Timecode(max_move_left),
            end - len + Timecode(max_move_right),
            playback_time,
            playback_time - len,
          ],
        );
        let start = drag_state.orig_timings[0].0;

        for (i, _ev) in drag_state.events.iter().enumerate() {
          let offset = drag_state.orig_timings[i].0 - start;
          drag_state.timings[i] = (
            drag_pos + offset,
            drag_pos + offset + (drag_state.orig_timings[i].1 - drag_state.orig_timings[i].0),
          );
        }
      }
    }
  }

  /// Finds the end of the previous event and start of the next event for the
  /// given event ID.
  fn get_event_track_bounds(
    project: &Project,
    event_ids: &HashSet<Uuid>,
    event_id: Uuid,
  ) -> (Timecode, Timecode) {
    for track in &project.file.tracks {
      for ev in track.events.iter() {
        if ev.id == event_id {
          return (
            track
              .events
              .iter()
              .filter(|e| !event_ids.contains(&e.id))
              .map(|e| e.end_timecode)
              .filter(|t| *t <= ev.start_timecode)
              .max()
              .unwrap_or(Timecode::MIN),
            track
              .events
              .iter()
              .filter(|e| !event_ids.contains(&e.id))
              .map(|e| e.start_timecode)
              .filter(|t| *t >= ev.end_timecode)
              .min()
              .unwrap_or(Timecode::MAX),
          );
        }
      }
    }

    (Timecode::MIN, Timecode::MAX)
  }

  fn get_drag_pos(
    pixels_per_second: f32,
    mouse_time: Timecode,
    min_time: Timecode,
    max_time: Timecode,
    snap_points: &[Timecode],
  ) -> Timecode {
    if mouse_time < min_time {
      return min_time;
    } else if mouse_time > max_time {
      return max_time;
    }

    let mouse_time_f32 = mouse_time.to_seconds();
    let snap_time = SNAP_WIDTH / pixels_per_second;
    for snap_point in snap_points {
      let snap_point_f32 = snap_point.to_seconds();
      if (snap_point_f32 - mouse_time_f32).abs() < (snap_time / 2.0) {
        return (*snap_point).clamp(min_time, max_time);
      }
    }

    mouse_time
  }
}
