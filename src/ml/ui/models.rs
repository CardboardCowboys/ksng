use egui::{Align, ProgressBar, Sense, Sides};
use egui_extras::{Column, TableBuilder};
use size::Size;
use uuid::Uuid;

use crate::ml::{models::ModelDownloadStatus, worker::WorkerManager};

#[derive(Default)]
pub struct ModelsTab {
  selected_model: Option<Uuid>,
}

impl ModelsTab {
  pub fn models_tab(&mut self, ui: &mut egui::Ui, worker: &WorkerManager) {
    let models_ref = worker.models.read().unwrap();
    let selected_model = if let Some(model_id) = self.selected_model {
      models_ref
        .models()
        .and_then(|m| m.iter().find(|m| m.id == model_id))
    } else {
      None
    };

    egui::Panel::bottom("models_window#tab_models_bottom")
      .min_size(100.0)
      .show(ui, |ui| {
        if let Some(model) = selected_model {
          Sides::new().show(
            ui,
            |ui| {
              ui.heading(&model.name);
            },
            |ui| {
              if matches!(
                model.download_status,
                ModelDownloadStatus::NotDownloaded | ModelDownloadStatus::Failed(..)
              ) && ui.button("Download").clicked()
              {
                models_ref.download_model(model.id);
              }
            },
          );
          ui.label(&model.description);
        }
      });

    egui::CentralPanel::default().show(ui, |ui| {
      let available_height = ui.available_height();

      let builder = TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .auto_shrink(false)
        .cell_layout(egui::Layout::left_to_right(Align::Center))
        .column(
          Column::remainder()
            .at_least(40.0)
            .clip(true)
            .resizable(true),
        )
        .column(Column::auto())
        .column(Column::auto())
        .column(Column::auto())
        .min_scrolled_height(0.0)
        .max_scroll_height(available_height)
        .sense(Sense::click());

      builder
        .header(20.0, |mut row| {
          row.col(|ui| {
            ui.label("Name");
          });
          row.col(|ui| {
            ui.label("Type");
          });
          row.col(|ui| {
            ui.label("Size");
          });
          row.col(|ui| {
            ui.label("Status");
          });
        })
        .body(|mut body| {
          let Some(models) = models_ref.models() else {
            return;
          };
          for model in models {
            body.row(18.0, |mut row| {
              if let Some(selected) = selected_model {
                row.set_selected(selected.id == model.id);
              }

              row.col(|ui| {
                ui.label(&model.name);
              });

              row.col(|ui| {
                ui.label(&model.model_type);
              });

              row.col(|ui| {
                let s = Size::from_bytes(model.size)
                  .format()
                  .with_base(size::Base::Base10)
                  .with_style(size::Style::Abbreviated)
                  .to_string();
                ui.label(&s);
              });

              row.col(|ui| {
                match &model.download_status {
                  ModelDownloadStatus::NotDownloaded => ui.label("Not Downloaded"),
                  ModelDownloadStatus::Downloaded => ui.label("Downloaded"),
                  ModelDownloadStatus::Downloading { progress } => {
                    ui.add(ProgressBar::new(*progress).show_percentage().animate(true))
                  }
                  ModelDownloadStatus::Failed(_) => ui.label("Failed"),
                };
              });

              if row.response().clicked() {
                self.selected_model = Some(model.id);
              }
            });
          }
        });
    });
  }
}
