use egui::{Color32, Id, Modal, ProgressBar, RichText};
use uuid::Uuid;

use crate::{ml::tasks::TaskStatus, modals::KModal};

pub struct TaskModal {
  task_id: Uuid,
  open: bool,
}

impl TaskModal {
  pub fn new(task_id: Uuid) -> TaskModal {
    TaskModal {
      task_id,
      open: true,
    }
  }
}

impl KModal for TaskModal {
  fn should_cleanup(&self) -> bool {
    !self.open
  }

  fn process(&mut self, app: &crate::KsngApp, context: &egui::Context) {
    if !self.open {
      return;
    }

    let mut should_close = false;

    Modal::new(Id::new("task_modal")).show(context, |ui| {
      ui.set_width(400.0);

      let tasks_ref = app.worker.tasks.read().unwrap();
      let Some(task) = tasks_ref.get_task(self.task_id) else {
        should_close = true;
        return;
      };

      let mut should_cancel = false;
      let mut should_finalize = false;

      ui.vertical_centered(|ui| {
        ui.heading(&task.name);
        let progress = match &task.status {
          TaskStatus::Running { progress } => *progress,
          TaskStatus::Complete(_) => 1.0_f32,
          _ => 0.0_f32,
        };
        ui.add(ProgressBar::new(progress).show_percentage().animate(true));
        match &task.status {
          TaskStatus::Running { .. } => {
            if ui.button("Cancel").clicked() {
              should_close = true;
              should_cancel = true;
              should_finalize = true;
            }
          }
          TaskStatus::Complete(_) => {
            ui.label("Complete!");
            if ui.button("Close").clicked() {
              should_close = true;
              should_finalize = true;
            }
          }
          TaskStatus::Failed(err) => {
            ui.label(RichText::new(err.as_str()).color(Color32::RED));
            if ui.button("Close").clicked() {
              should_close = true;
              should_finalize = true;
            }
          }
          TaskStatus::Cancelled => {
            should_close = true;
          }
        }
      });

      drop(tasks_ref);

      if should_finalize {
        let mut tasks_ref = app.worker.tasks.write().unwrap();
        tasks_ref.finalize_task(self.task_id);
      }

      if should_cancel {
        let mut tasks_ref = app.worker.tasks.write().unwrap();
        tasks_ref.cancel_task(self.task_id);
      }
    });

    if should_close {
      self.open = false;
    }
  }
}
