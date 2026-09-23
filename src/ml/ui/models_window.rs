use egui::Window;
use egui_dock::{DockArea, DockState, Style, TabViewer};

use crate::{
  KsngApp,
  ml::{
    ui::{htdemucs::HtdemucsTab, models::ModelsTab},
    worker::WorkerManager,
  },
  windows::KWindow,
};

#[derive(Debug)]
enum ModelsWindowTab {
  Models,
  StemSeparation,
}

struct ModelsWindowTabViewer<'worker> {
  worker: &'worker WorkerManager,
  models: &'worker mut ModelsTab,
  htdemucs: &'worker mut HtdemucsTab,
  app: &'worker KsngApp,
}

impl<'worker> TabViewer for ModelsWindowTabViewer<'worker> {
  type Tab = ModelsWindowTab;

  fn id(&mut self, tab: &mut Self::Tab) -> egui::Id {
    egui::Id::new(format!("models_window#tab_{tab:?}"))
  }

  fn title(&mut self, tab: &mut Self::Tab) -> egui_dock::egui::WidgetText {
    match tab {
      ModelsWindowTab::StemSeparation => "Stem Separation",
      ModelsWindowTab::Models => "Models",
    }
    .into()
  }

  fn ui(&mut self, ui: &mut egui_dock::egui::Ui, tab: &mut Self::Tab) {
    match tab {
      ModelsWindowTab::StemSeparation => self.htdemucs.htdemucs_tab(self.app, ui, self.worker),
      ModelsWindowTab::Models => self.models.models_tab(ui, self.worker),
    }
  }
}

pub struct ModelsWindow {
  open: bool,
  should_request_focus: bool,
  dock_state: DockState<ModelsWindowTab>,
  models_tab: ModelsTab,
  htdemucs_tab: HtdemucsTab,
}

impl ModelsWindow {
  pub fn new() -> ModelsWindow {
    ModelsWindow {
      open: true,
      should_request_focus: true,
      dock_state: DockState::new(vec![
        ModelsWindowTab::StemSeparation,
        ModelsWindowTab::Models,
      ]),
      models_tab: ModelsTab::default(),
      htdemucs_tab: HtdemucsTab::default(),
    }
  }
}

impl KWindow for ModelsWindow {
  fn should_cleanup(&self) -> bool {
    !self.open
  }

  fn process(&mut self, app: &crate::KsngApp, context: &egui::Context) {
    if !self.open {
      return;
    }

    let window = Window::new("Models").show(context, |ui| {
      if !matches!(app.logger.wrap(WorkerManager::is_installed()), Some(true)) {
        egui::CentralPanel::default().show(ui, |ui| {
          ui.vertical_centered(|ui| {
            ui.label("ksng-ml has not been installed!");
          })
        });
        return;
      }

      if !app.worker.is_running() {
        egui::CentralPanel::default().show(ui, |ui| {
          ui.vertical_centered(|ui| {
            ui.label("ksng-ml is not running");
            if ui.button("Start").clicked() {
              app.logger.wrap(app.worker.start());
            }
          });
        });
        return;
      }

      DockArea::new(&mut self.dock_state)
        .style(Style::from_egui(ui.style().as_ref()))
        .show_close_buttons(false)
        .show_inside(
          ui,
          &mut ModelsWindowTabViewer {
            worker: &app.worker,
            models: &mut self.models_tab,
            htdemucs: &mut self.htdemucs_tab,
            app,
          },
        );
    });

    if let Some(window) = window
      && self.should_request_focus
    {
      window.response.request_focus();
      self.should_request_focus = false;
    }
  }

  fn request_focus(&mut self) {
    self.should_request_focus = true;
  }

  fn unique_value(&self) -> Option<u64> {
    Some(0xb61aefc318a82ccc)
  }
}
