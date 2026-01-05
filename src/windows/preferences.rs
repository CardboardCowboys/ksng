use std::{
  hash::{DefaultHasher, Hash, Hasher},
  str::FromStr,
};

use cpal::{HostId, traits::DeviceTrait};
use egui::{Button, ComboBox, Sides, Window};

use crate::{
  audio::config::AudioConfig, fs::Data, preferences::Preferences, util::ui_event::KsngEvent,
  windows::KWindow,
};

pub struct PreferencesWindow {
  open: bool,
  dirty: bool,
  preferences: Preferences,
  unique_value: u64,
  should_request_focus: bool,
}

impl PreferencesWindow {
  pub fn new(preferences: Preferences) -> Self {
    let mut hasher = DefaultHasher::new();
    "PreferencesWindow".hash(&mut hasher);

    PreferencesWindow {
      open: true,
      dirty: false,
      unique_value: hasher.finish(),
      should_request_focus: false,
      preferences,
    }
  }
}

impl KWindow for PreferencesWindow {
  fn should_cleanup(&self) -> bool {
    !self.open
  }

  fn process(&mut self, app: &crate::KsngApp, context: &egui::Context) {
    if !self.open {
      return;
    }

    let window = Window::new("Preferences").show(context, |ui| {
      ui.set_width(250.0);

      let mut changed = false;
      egui::Grid::new("window#preferences_editor")
        .num_columns(2)
        .spacing([40.0, 2.0])
        .striped(true)
        .show(ui, |ui| {
          ui.label("Audio");
          ui.vertical(|ui| {
            let audio_config = &mut self.preferences.audio_config;
            ComboBox::new("preferences#audio_host", "Host")
              .selected_text(&audio_config.host)
              .show_ui(ui, |ui| {
                for host in AudioConfig::hosts() {
                  ui.selectable_value(&mut audio_config.host, host.clone(), host);
                }
              });
            if audio_config.host != app.preferences.borrow().audio_config.host {
              changed = true;
            }

            let mut device = audio_config.device.clone().unwrap_or_default();
            ComboBox::new("preferences#audio_device", "Device")
              .selected_text(
                audio_config
                  .device
                  .as_ref()
                  .and_then(|d| AudioConfig::device_name(&audio_config.host, d))
                  .unwrap_or_default(),
              )
              .show_ui(ui, |ui| {
                let Some(host) = HostId::from_str(&audio_config.host).ok() else {
                  return;
                };
                for d in AudioConfig::devices(host) {
                  let Some(id) = d.id().ok().map(|d| d.1) else {
                    continue;
                  };
                  let Some(name) = AudioConfig::device_name(&audio_config.host, &id) else {
                    continue;
                  };
                  ui.selectable_value(&mut device, id, name);
                }
              });
            if device.is_empty() {
              audio_config.device = None;
            } else {
              audio_config.device = Some(device);
            }

            if audio_config.device != app.preferences.borrow().audio_config.device {
              changed = true;
            }
          });
          ui.end_row();
        });

      if changed {
        self.dirty = true;
      }

      Sides::new().show(
        ui,
        |_ui| {},
        |ui| {
          if ui.button("Cancel").clicked() {
            self.open = false;
          }

          if ui.add_enabled(self.dirty, Button::new("Apply")).clicked() {
            self.dirty = false;
            *app.preferences.borrow_mut() = self.preferences.clone();
            app
              .logger
              .wrap(Data::save_preferences(&app.preferences.borrow()));
            app.dispatch(KsngEvent::AudioDeviceChanged);
          }

          if ui.add_enabled(self.dirty, Button::new("OK")).clicked() {
            self.dirty = false;
            self.open = false;
            *app.preferences.borrow_mut() = self.preferences.clone();
            app
              .logger
              .wrap(Data::save_preferences(&app.preferences.borrow()));
            app.dispatch(KsngEvent::AudioDeviceChanged);
          }
        },
      )
    });

    if let Some(window) = window {
      if self.should_request_focus {
        window.response.request_focus();
        self.should_request_focus = false;
      }
    }
  }

  fn request_focus(&mut self) {
    self.should_request_focus = true;
  }

  fn unique_value(&self) -> Option<u64> {
    Some(self.unique_value)
  }
}
