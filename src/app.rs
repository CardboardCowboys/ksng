use std::{
  cell::RefCell,
  collections::VecDeque,
  ops::Deref,
  sync::{Arc, RwLock},
};

use egui::{Button, Context, Sides};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
  async_handler::AsyncHandler,
  data::Data,
  logger::Logger,
  modals::{
    dirty_warning::DirtyWarningModal, open_project::OpenProjectModal,
    save_project::SaveProjectModal, ModalManager,
  },
  project::Project,
  ui_event::KsngEvent,
};

pub struct ProjectHolder {
  pub project: RwLock<Option<Project>>,
}

impl ProjectHolder {
  pub fn new() -> ProjectHolder {
    ProjectHolder {
      project: Default::default(),
    }
  }
}

impl Deref for ProjectHolder {
  type Target = RwLock<Option<Project>>;

  fn deref(&self) -> &Self::Target {
    &self.project
  }
}

pub struct KsngApp {
  pub project: Arc<ProjectHolder>,
  pub modals: ModalManager,
  pub logger: Arc<Logger>,
  pub async_handler: Arc<AsyncHandler>,
  pub data: Arc<Data>,

  pub event_queue: Arc<RwLock<VecDeque<KsngEvent>>>,
  close_allowed: RefCell<bool>,
}

#[derive(Serialize, Deserialize, Default)]
struct AppSavedData {
  project_id: Option<Uuid>,
}

impl Default for KsngApp {
  fn default() -> Self {
    let logger = Arc::new(Logger::default());
    Self {
      project: Arc::new(ProjectHolder::new()),
      modals: Default::default(),
      async_handler: Arc::new(AsyncHandler::new(logger.clone())),
      data: Arc::new(Data::new(logger.clone())),
      logger,
      event_queue: Default::default(),
      close_allowed: RefCell::new(false),
    }
  }
}

impl KsngApp {
  fn on_event(&self, ctx: &Context, event: KsngEvent) {
    match event {
      KsngEvent::ProjectClose => {
        (*self.project.write().expect("Poisoned")) = None;
      }
      KsngEvent::ProjectNew => {
        self
          .project
          .write()
          .expect("Poisoned")
          .replace(Project::default());
      }
      KsngEvent::ProjectSave => {
        SaveProjectModal::save(self, None);
      }
      KsngEvent::ProjectOpen => {
        self.modals.add(OpenProjectModal::new());
      }
      KsngEvent::ProjectOpenId(id) => {
        let data = self.data.clone();
        let project_holder = self.project.clone();
        self.async_handler.clone().wrap(async move || {
          let project = data.load_project(id).await?;
          project_holder.write().expect("Poisoned").replace(project);

          Ok(())
        });
      }
      KsngEvent::Quit => {
        *self.close_allowed.borrow_mut() = true;
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
      }
      KsngEvent::ProjectDelete(id) => {
        let data = self.data.clone();
        self
          .async_handler
          .clone()
          .wrap(async move || data.delete_project(id).await);
      }
    }
  }

  pub fn dispatch(&self, event: KsngEvent) {
    self.event_queue.write().expect("Poisoned").push_back(event);
  }

  pub fn dispatch_warn_dirty(&self, event: KsngEvent) {
    if let Some(project) = self.project.read().expect("Poisoned").as_ref() {
      if project.dirty {
        self.modals.add(DirtyWarningModal::new(event));
        return;
      }
    }

    self.dispatch(event);
  }

  pub fn set_dirty_state(&self, dirty: bool) {
    if let Some(project) = self.project.write().expect("Poisoned").as_mut() {
      project.dirty = dirty;
    }
  }
}

impl KsngApp {
  /// Called once before the first frame.
  pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
    // This is also where you can customize the look and feel of egui using
    // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

    let app = KsngApp::default();

    if let Some(storage) = cc.storage {
      let data: AppSavedData = eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
      if let Some(project_id) = data.project_id {
        let data = app.data.clone();
        let project_holder = app.project.clone();
        app.async_handler.clone().wrap(async move || {
          let project = data.load_project(project_id).await?;
          project_holder.write().expect("Poisoned").replace(project);
          Ok(())
        });
      }
    }

    egui_extras::install_image_loaders(&cc.egui_ctx);

    app
  }
}

impl eframe::App for KsngApp {
  /// Called by the frame work to save state before shutdown.
  fn save(&mut self, storage: &mut dyn eframe::Storage) {
    let data = AppSavedData {
      project_id: self
        .project
        .read()
        .expect("Poisoned")
        .as_ref()
        .map(|p| Some(p.id))
        .unwrap_or(None),
    };
    eframe::set_value(storage, eframe::APP_KEY, &data);
  }

  /// Called each time the UI needs repainting, which may be many times per second.
  fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
    let mut queue = self.event_queue.write().expect("Poisoned");
    while let Some(event) = queue.pop_front() {
      self.on_event(ctx, event);
    }
    drop(queue);

    // Put your widgets into a `SidePanel`, `TopBottomPanel`, `CentralPanel`, `Window` or `Area`.
    // For inspiration and more examples, go to https://emilk.github.io/egui

    egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
      egui::menu::bar(ui, |ui| {
        let project = self.project.read().expect("Poisoned");
        Sides::new().show(
          ui,
          |ui| {
            let is_web = cfg!(target_arch = "wasm32");
            ui.menu_button("File", |ui| {
              if ui.button("New").clicked() {
                self.dispatch_warn_dirty(KsngEvent::ProjectNew);
              }

              if ui.button("Open").clicked() {
                self.dispatch_warn_dirty(KsngEvent::ProjectOpen);
              }

              let is_dirty = project.as_ref().map(|f| f.dirty).unwrap_or(false);
              if ui.add_enabled(is_dirty, Button::new("Save")).clicked() {
                self.dispatch(KsngEvent::ProjectSave);
              }

              if ui
                .add_enabled(project.is_some(), Button::new("Close"))
                .clicked()
              {
                self.dispatch_warn_dirty(KsngEvent::ProjectClose);
              }

              // NOTE: no File->Quit on web pages!
              if !is_web && ui.button("Quit").clicked() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
              }
            });
          },
          |ui| {
            if let Some(project) = project.as_ref() {
              ui.label(
                format!(
                  "Project: {}",
                  project.name.as_ref().unwrap_or(&"(unnamed)".to_string())
                ) + match project.dirty {
                  true => "*",
                  false => "",
                },
              );
            } else {
              ui.label("No project");
            }
          },
        )
      });
    });

    self.modals.process(self, ctx);

    egui::CentralPanel::default().show(ctx, |ui| {
      // The central panel the region left after adding TopPanel's and SidePanel's
      ui.heading("eframe template");

      ui.separator();

      ui.add(egui::github_link_file!(
        "https://github.com/emilk/eframe_template/blob/main/",
        "Source code."
      ));

      ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
        powered_by_egui_and_eframe(ui);
        egui::warn_if_debug_build(ui);
      });
    });

    self.async_handler.update(ctx);

    // Handle exit event, blocking if unsaved.
    if ctx.input(|i| i.viewport().close_requested()) {
      if let Some(project) = self.project.read().expect("Poisoned").as_ref() {
        if project.dirty && !*self.close_allowed.borrow() {
          ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
          self.dispatch_warn_dirty(KsngEvent::Quit);
        }
      }
    }
  }
}

fn powered_by_egui_and_eframe(ui: &mut egui::Ui) {
  ui.horizontal(|ui| {
    ui.spacing_mut().item_spacing.x = 0.0;
    ui.label("Powered by ");
    ui.hyperlink_to("egui", "https://github.com/emilk/egui");
    ui.label(" and ");
    ui.hyperlink_to(
      "eframe",
      "https://github.com/emilk/egui/tree/master/crates/eframe",
    );
    ui.label(".");
  });
}
