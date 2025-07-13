use std::{cell::RefCell, collections::VecDeque};

use egui::{Context, Id};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
  commands::CommandDispatcher,
  components::{self, timeline::Timeline},
  fs::Data,
  logger::Logger,
  modals::{
    dirty_warning::DirtyWarningModal, open_project::OpenProjectModal,
    save_project::SaveProjectModal, ModalManager,
  },
  project::Project,
  selection::SelectionManager,
  ui_event::KsngEvent,
};

pub struct KsngApp {
  pub project: RefCell<Option<Project>>,
  pub modals: ModalManager,
  pub logger: Logger,
  pub commands: CommandDispatcher,
  pub selection: SelectionManager,

  event_queue: RefCell<VecDeque<KsngEvent>>,
  close_allowed: RefCell<bool>,
  timeline: RefCell<Timeline>,
}

#[derive(Serialize, Deserialize, Default)]
struct AppSavedData {
  project_id: Option<Uuid>,
}

impl Default for KsngApp {
  fn default() -> Self {
    Self {
      project: RefCell::new(None),
      modals: Default::default(),
      logger: Default::default(),
      commands: CommandDispatcher::default(),
      selection: SelectionManager::default(),
      event_queue: Default::default(),
      close_allowed: RefCell::new(false),
      timeline: Default::default(),
    }
  }
}

impl KsngApp {
  fn on_event(&self, ctx: &Context, event: KsngEvent) {
    match event {
      KsngEvent::ProjectClose => {
        self.project.replace(None);
        self.selection.clear();
        *self.timeline.borrow_mut() = Timeline::default();
      }
      KsngEvent::ProjectNew => {
        self.project.replace(Some(Project::default()));
        self.selection.clear();
        *self.timeline.borrow_mut() = Timeline::default();
      }
      KsngEvent::ProjectSave => {
        SaveProjectModal::save(self, None);
      }
      KsngEvent::ProjectOpen => {
        self.modals.add(OpenProjectModal::new());
      }
      KsngEvent::ProjectOpenId(id) => {
        let project = self
          .logger
          .wrap(Data::list_projects().and_then(|manifest| Data::load_project(id, &manifest)));

        if let Some(project) = project {
          self.project.replace(Some(project));
          self.selection.clear();
          *self.timeline.borrow_mut() = Timeline::default();
        }
      }
      KsngEvent::Quit => {
        *self.close_allowed.borrow_mut() = true;
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
      }
      KsngEvent::ProjectDelete(id) => {
        self.logger.wrap(Data::delete_project(id));
      }
      KsngEvent::Undo => {
        self.logger.wrap(self.commands.undo(self));
      }
      KsngEvent::Redo => {
        self.logger.wrap(self.commands.redo(self));
      }
    }
  }

  pub fn dispatch(&self, event: KsngEvent) {
    self.event_queue.borrow_mut().push_back(event);
  }

  pub fn dispatch_warn_dirty(&self, event: KsngEvent) {
    if let Some(project) = &*self.project.borrow() {
      if project.dirty {
        self.modals.add(DirtyWarningModal::new(event));
        return;
      }
    }

    self.dispatch(event);
  }

  pub fn set_dirty_state(&self, dirty: bool) {
    if let Some(project) = self.project.borrow_mut().as_mut() {
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
        let project = app.logger.wrap(
          Data::list_projects().and_then(|manifest| Data::load_project(project_id, &manifest)),
        );
        app.project.replace(project);
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
        .borrow()
        .as_ref()
        .map(|p| Some(p.id))
        .unwrap_or(None),
    };
    eframe::set_value(storage, eframe::APP_KEY, &data);
  }

  /// Called each time the UI needs repainting, which may be many times per second.
  fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
    let mut queue = self.event_queue.borrow_mut();
    while let Some(event) = queue.pop_front() {
      self.on_event(ctx, event);
    }
    drop(queue);

    self.logger.wrap(self.commands.process(self));
    self.modals.process(self, ctx);

    // Put your widgets into a `SidePanel`, `TopBottomPanel`, `CentralPanel`, `Window` or `Area`.
    // For inspiration and more examples, go to https://emilk.github.io/egui

    egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
      components::menu_bar::menu_bar(self, ctx, ui);
    });

    egui::CentralPanel::default().show(ctx, |ui| {
      egui::TopBottomPanel::bottom(Id::new("timeline"))
        .default_height(200.0)
        .resizable(true)
        .show_inside(ui, |ui| {
          self.timeline.borrow_mut().update(self, ctx, ui);
        });
    });

    if ctx.input(|i| i.viewport().close_requested()) {
      if let Some(project) = self.project.borrow().as_ref() {
        if project.dirty && !*self.close_allowed.borrow() {
          ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
          self.dispatch_warn_dirty(KsngEvent::Quit);
        }
      }
    }
  }
}
