use egui::{AsIdSalt, ComboBox, Ui, WidgetText};
use uuid::Uuid;

use crate::ml::worker::WorkerManager;

mod htdemucs;
mod models;
pub mod models_window;
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
