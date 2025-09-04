use egui::{
  load::SizedTexture, Align, Context, Id, ImageButton, ImageSource, Layout, Slider, TopBottomPanel,
  Ui, Vec2,
};
use klib::timecode::Timecode;

use crate::{playback::PlaybackState, style::icons, KsngApp};

pub fn player(app: &KsngApp, ctx: &Context, ui: &mut Ui) {
  let rect = ui.max_rect();
  TopBottomPanel::top(Id::new("player#view")).show_inside(ui, |ui| {
    if let Some(texture) = app.video.borrow().last_frame_texture() {
      ui.with_layout(Layout::top_down(Align::Center), |ui| {
        ui.add(
          egui::Image::new(ImageSource::Texture(texture)).fit_to_exact_size(Vec2::new(
            (rect.width() - 20.0).max(1.0),
            (rect.height() - 20.0).max(1.0),
          )),
        );
      });
    }
  });
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

        let mut slider_position = position.0;
        ui.style_mut().spacing.slider_width = ui.max_rect().width();
        ui.add(
          Slider::new(&mut slider_position, 0..=duration.0)
            .show_value(false)
            .integer(),
        );

        if slider_position != position.0 {
          app.playback.borrow().seek(Timecode(slider_position));
        }

        ui.columns(3, |columns| {
          columns[0].with_layout(Layout::top_down(Align::Min), |ui| ui.label("00:00"));
          columns[1].with_layout(Layout::top_down(Align::Center), |ui| {
            ui.label(position.to_string_seconds());
          });
          columns[2].with_layout(Layout::top_down(Align::Max), |ui| {
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
