use egui::{
  Align, Context, Id, ImageButton, ImageSource, Layout, Rect, Slider, TopBottomPanel, Ui, Vec2,
};
use klib::timecode::Timecode;

use crate::{KsngApp, playback::PlaybackState, style::icons};

fn calculate_video_size(available_size: Vec2, video_size: Vec2) -> Vec2 {
  let x_scale = available_size.x / video_size.x;
  let y_scale = available_size.y / video_size.y;
  let scale = x_scale.min(y_scale);
  Vec2::new(scale * video_size.x, scale * video_size.y)
}

pub fn player(app: &KsngApp, _ctx: &Context, ui: &mut Ui) {
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
          app.playback.borrow_mut().seek(Timecode(slider_position));
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
  let size = ui.available_size();
  TopBottomPanel::top(Id::new("player#view")).show_inside(ui, |ui| {
    if let Some(texture) = app.video.borrow().last_frame_texture() {
      let size = calculate_video_size(size, texture.size);
      ui.with_layout(Layout::top_down(Align::Center), |ui| {
        ui.add(
          egui::Image::new(ImageSource::Texture(texture)).fit_to_exact_size(Vec2::new(
            (size.x - 20.0).max(1.0),
            (size.y - 20.0).max(1.0),
          )),
        );
      });
    }
  });
}
