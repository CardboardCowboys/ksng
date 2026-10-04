use egui::{AsIdSalt, ComboBox, Ui, WidgetText};
use uuid::Uuid;

use crate::{KsngContext, Logger, ml::worker::WorkerManager};

pub mod htdemucs;
pub mod models;
mod task;

fn model_select_dropdown(
  ui: &mut Ui,
  id: impl AsIdSalt,
  label: impl Into<WidgetText>,
  selected: Option<(Uuid, &str)>,
  worker: &WorkerManager,
  model_type: &str,
) -> Option<(Uuid, String)> {
  let models_ref = worker.models.read().unwrap();
  let models = models_ref.models();

  let mut current = selected.map(|s| s.0).unwrap_or_default();

  ui.add_enabled_ui(models.is_some(), |ui| {
    ComboBox::new(id, label)
      .selected_text(selected.map(|s| s.1).unwrap_or(""))
      .show_ui(ui, |ui| {
        if let Some(models) = models {
          for model in models {
            if model.model_type == model_type {
              ui.selectable_value(&mut current, model.id, &model.name);
            }
          }
        }
      });
  });

  if let Some(models) = models
    && current != Uuid::default()
  {
    models
      .iter()
      .find(|m| m.id == current)
      .map(|m| (m.id, m.name.clone()))
  } else {
    None
  }
}

fn ksng_ml_status_panel(ui: &mut Ui, app: &KsngContext) -> bool {
  if !matches!(Logger::wrap(WorkerManager::is_installed()), Some(true)) {
    egui::CentralPanel::default().show(ui, |ui| {
      ui.vertical_centered(|ui| {
        ui.label("ksng-ml has not been installed!");
      })
    });
    return false;
  }

  if !app.worker.is_running() {
    egui::CentralPanel::default().show(ui, |ui| {
      ui.vertical_centered(|ui| {
        ui.label("ksng-ml is not running");
        if ui.button("Start").clicked() {
          Logger::wrap(app.worker.start());
        }
      });
    });
    return false;
  }

  true
}
