use egui::{CentralPanel, Context, Frame, Grid, Id, Margin, ScrollArea, SidePanel, Stroke, Ui};

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
          Frame::new()
            .corner_radius(0)
            .outer_margin(Margin::symmetric(0, TRACK_INNER_PADDING))
            .inner_margin(5)
            .fill(color_for_track_type(track.track_type))
            .show(ui, |ui| {
              ui.set_height(TRACK_HEIGHT);
              ui.set_width(TRACK_HEADER_WIDTH);
              ui.heading(format!("{:?}", track.track_type));
            });
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
