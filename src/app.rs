use std::{cell::RefCell, collections::VecDeque};

use egui::{Context, Id};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
  audio::waveform::AudioWaveformProvider,
  commands::CommandDispatcher,
  components::{self, lyrics_editor::LyricsEditor, timeline::Timeline},
  fs::Data,
  ml::worker::WorkerManager,
  modals::{
    ModalManager, dirty_warning::DirtyWarningModal, export_video::ExportVideoModal,
    open_project::OpenProjectModal, save_project::SaveProjectModal,
  },
  playback::{Playback, PlaybackState},
  preferences::Preferences,
  project::Project,
  selection::SelectionManager,
  util::{logger::Logger, ui_event::KsngEvent},
  video::VideoState,
  windows::WindowManager,
};

pub struct KsngApp {
  pub project: RefCell<Option<Project>>,
  pub modals: ModalManager,
  pub windows: WindowManager,
  pub logger: Logger,
  pub commands: CommandDispatcher,
  pub selection: SelectionManager,
  pub waveforms: RefCell<AudioWaveformProvider>,
  pub playback: RefCell<Playback>,
  pub video: RefCell<VideoState>,
  pub lyrics_editor: RefCell<LyricsEditor>,
  pub worker: WorkerManager,

  pub preferences: RefCell<Preferences>,

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
    let logger = Logger::default();
    let preferences = Preferences::default();
    Self {
      project: RefCell::new(None),
      modals: Default::default(),
      windows: Default::default(),
      waveforms: RefCell::new(AudioWaveformProvider::new(logger.clone())),
      playback: Playback::new(&preferences.audio_config, logger.clone()).into(),
      video: RefCell::new(VideoState::new().unwrap()),
      worker: WorkerManager::new(logger.clone()),
      logger,
      commands: CommandDispatcher::default(),
      selection: SelectionManager::default(),
      event_queue: Default::default(),
      close_allowed: RefCell::new(false),
      timeline: Default::default(),
      preferences: RefCell::new(preferences),
      lyrics_editor: Default::default(),
    }
  }
}

impl KsngApp {
  fn on_project_change(&self, ctx: &Context) {
    self.selection.clear();
    self.windows.clear();
    *self.timeline.borrow_mut() = Timeline::default();
    self.waveforms.borrow_mut().clear(ctx);
    self.playback.borrow_mut().on_audio_change(self);
    if let Some(project) = self.project.borrow().as_ref() {
      self.video.borrow_mut().update_from_file(&project.file);
    } else {
      self.video.borrow_mut().clear();
    }
    self.lyrics_editor.borrow_mut().on_project_change(self);
  }

  fn on_event(&self, ctx: &Context, event: KsngEvent) {
    match event {
      KsngEvent::ProjectClose => {
        self.project.replace(None);
        self.on_project_change(ctx);
      }
      KsngEvent::ProjectNew => {
        self.project.replace(Some(Project::default()));
        self.on_project_change(ctx);
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
          self.on_project_change(ctx);
        }
      }
      KsngEvent::ProjectExportVideo => {
        let mut name = "none".to_string();
        if let Some(project) = &*self.project.borrow() {
          name = project.name.clone().unwrap_or("none".to_string());
        }
        self.modals.add(ExportVideoModal::new(name));
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
      KsngEvent::AudioDeviceChanged => {
        self.playback.borrow_mut().on_audio_device_change(self);
      }
      KsngEvent::CloseWindow(unique) => {
        self.windows.close_window(unique);
      }
    }
  }

  pub fn dispatch(&self, event: KsngEvent) {
    self.event_queue.borrow_mut().push_back(event);
  }

  pub fn dispatch_warn_dirty(&self, event: KsngEvent) {
    if let Some(project) = &*self.project.borrow()
      && project.dirty
    {
      self.modals.add(DirtyWarningModal::new(event));
      return;
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
    let app = KsngApp::default();

    if let Some(storage) = cc.storage {
      let data: AppSavedData = eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
      if let Some(project_id) = data.project_id {
        let project = app.logger.wrap(
          Data::list_projects().and_then(|manifest| Data::load_project(project_id, &manifest)),
        );
        app.project.replace(project);
        app.on_project_change(&cc.egui_ctx);
      }
      let preferences = app
        .logger
        .wrap(Data::load_preferences())
        .unwrap_or_default();
      app.preferences.replace(preferences);
      app.playback.borrow_mut().on_audio_device_change(&app);

      if let Some(b) = app.logger.wrap(WorkerManager::is_installed())
        && b
      {
        app.logger.wrap(app.worker.start());
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

  fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
    let mut queue = self.event_queue.borrow_mut();
    while let Some(event) = queue.pop_front() {
      self.on_event(ctx, event);
    }
    drop(queue);

    self.logger.wrap(self.commands.process(self));
    self.modals.process(self, ctx);
    self.windows.process(self, ctx);
    self.playback.borrow_mut().update();
    self.worker.tasks.write().unwrap().poll_tasks(self);

    self.logger.wrap(
      self
        .video
        .borrow_mut()
        .process_frame(ctx, self.playback.borrow().position()),
    );
  }

  /// Called each time the UI needs repainting, which may be many times per
  /// second.
  fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
    let ctx = ui.ctx();

    if self.playback.borrow().state() == PlaybackState::Playing {
      ctx.request_repaint();
    }

    egui::Panel::top("top_panel").show(ui, |ui| {
      components::menu_bar::menu_bar(self, ui);
    });

    egui::CentralPanel::default().show(ui, |ui| {
      egui::Panel::bottom(Id::new("timeline"))
        .default_size(200.0)
        .resizable(true)
        .show(ui, |ui| {
          self.timeline.borrow_mut().update(self, ui);
        });
      egui::CentralPanel::default().show(ui, |ui| {
        egui::Panel::right(Id::new("player"))
          .default_size(300.0)
          .max_size((ui.available_width() - 100.0).max(0.0))
          .resizable(true)
          .show(ui, |ui| {
            components::player::player(self, ui);
          });
        egui::CentralPanel::default()
          .frame(egui::Frame::new().inner_margin(0.0))
          .show(ui, |ui| {
            self.lyrics_editor.borrow_mut().show(self, ui);
          });
      });
    });

    let ctx = ui.ctx();

    if ctx.input(|i| i.viewport().close_requested())
      && let Some(project) = self.project.borrow().as_ref()
      && project.dirty
      && !*self.close_allowed.borrow()
    {
      ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
      self.dispatch_warn_dirty(KsngEvent::Quit);
    }
  }
}
