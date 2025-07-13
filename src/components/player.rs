use egui::{Align, Context, Id, ImageButton, Layout, Slider, TopBottomPanel, Ui, Vec2};

use crate::{playback::PlaybackState, style::icons, KsngApp};

pub fn player(app: &KsngApp, ctx: &Context, ui: &mut Ui) {
  TopBottomPanel::bottom(Id::new("player#controls")).show_inside(ui, |ui| {
    ui.add_enabled_ui(app.project.borrow().is_some(), |ui| {
      ui.vertical_centered(|ui| {
        let position = app.playback.borrow().position();
        let duration = app
          .project
          .borrow()
          .as_ref()
          .map(|p| p.length)
          .unwrap_or_default();
        ui.columns(3, |columns| {
          columns[0].with_layout(Layout::top_down(Align::Center), |ui| ui.label("00:00"));
          columns[1].with_layout(Layout::top_down(Align::Center), |ui| {
            ui.label(position.to_string_seconds());
          });
          columns[2].with_layout(Layout::top_down(Align::Center), |ui| {
            ui.label(duration.to_string_seconds());
          });
        });

        let state = app.playback.borrow().state();
        let button = if state == PlaybackState::Playing {
          ImageButton::new(icons::PAUSE)
        } else {
          ImageButton::new(icons::PLAY)
        };

        if ui.add_sized(Vec2::new(40.0, 40.0), button).clicked() {
          app.playback.borrow_mut().toggle_state();
        }
      });
    });
  });
}
