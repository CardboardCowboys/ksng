use egui::{
  Area, Button, CentralPanel, Color32, Context, Frame, Grid, Id, Margin, PointerButton, ScrollArea,
  Sense, SidePanel, Stroke, Ui,
};

use crate::{style::colors::color_for_track_type, KsngApp};

const TRACK_HEIGHT: f32 = 50.0;
const TRACK_INNER_PADDING: i8 = 2;
const TRACK_HEADER_WIDTH: f32 = 200.0;

pub fn timeline(app: &KsngApp, ctx: &Context, ui: &mut Ui) {
  ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
    let project = app.project.borrow();
    if project.is_none() {
      return;
    }
    let project = project.as_ref().unwrap();

    Grid::new(Id::new("timeline#grid"))
      .min_row_height(TRACK_HEIGHT)
      .min_col_width(200.0)
      .striped(true)
      .show(ui, |ui| {
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

          CentralPanel::default()
            .frame(
              Frame::new()
                .inner_margin(Margin::symmetric(TRACK_INNER_PADDING, TRACK_INNER_PADDING)),
            )
            .show_inside(ui, |ui| {
              ui.label("hello world");
            });
          ui.end_row();
        }
      });
  });
}
