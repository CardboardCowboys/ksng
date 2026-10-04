use std::str::FromStr;

use cpal::{HostId, traits::DeviceTrait};
use egui::{Button, ComboBox, Sides, Ui};

use crate::{
  audio::config::AudioConfig, fs::Data, preferences::Preferences, util::ui_event::KsngEvent,
};

#[derive(Clone, Debug)]
pub struct PreferencesWindow {
  dirty: bool,
  preferences: Preferences,
  old_preferences: Preferences,
}

impl PartialEq for PreferencesWindow {
  fn eq(&self, _other: &Self) -> bool {
    true
  }
}

impl PreferencesWindow {
  pub fn new(preferences: Preferences) -> Self {
    PreferencesWindow {
      dirty: false,
      old_preferences: preferences.clone(),
      preferences,
    }
  }
}

impl PreferencesWindow {
  pub fn process(&mut self, app: &crate::KsngContext, ui: &mut Ui) {
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
        if ui.add_enabled(self.dirty, Button::new("Reset")).clicked() {
          self.preferences = self.old_preferences.clone();
          self.dirty = false;
        }

        if ui.add_enabled(self.dirty, Button::new("Apply")).clicked() {
          self.dirty = false;
          *app.preferences.borrow_mut() = self.preferences.clone();
          app
            .logger
            .wrap(Data::save_preferences(&app.preferences.borrow()));
          app.dispatch(KsngEvent::AudioDeviceChanged);
          self.old_preferences = self.preferences.clone();
        }
      },
    );
  }
}
