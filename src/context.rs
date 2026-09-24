use std::{cell::RefCell, collections::VecDeque};

use eframe::Storage;
use egui::{Context, Ui};
use egui_dock::DockState;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
  app::{AppTab, AppTabInitializer},
  audio::waveform::AudioWaveformProvider,
  commands::CommandDispatcher,
  components::{lyrics_editor::LyricsEditor, timeline::Timeline},
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
  windows::{WindowManager, preferences::PreferencesWindow, track_config::TrackConfigWindow},
};

pub struct KsngContext {
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
  pub timeline: RefCell<Timeline>,
  close_allowed: RefCell<bool>,
}

impl Default for KsngContext {
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
      timeline: Default::default(),
      preferences: RefCell::new(preferences),
      lyrics_editor: Default::default(),
      close_allowed: RefCell::new(false),
    }
  }
}

#[derive(Serialize, Deserialize, Default)]
struct AppSavedData {
  project_id: Option<Uuid>,
}

impl KsngContext {
  pub fn load_storage(&self, storage: &dyn Storage, ctx: &Context) {
    let data: AppSavedData = eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
    if let Some(project_id) = data.project_id {
      let project = self
        .logger
        .wrap(Data::list_projects().and_then(|manifest| Data::load_project(project_id, &manifest)));
      self.project.replace(project);
      self.on_project_change(ctx, None);
    }
    let preferences = self
      .logger
      .wrap(Data::load_preferences())
      .unwrap_or_default();
    self.preferences.replace(preferences);
    self.playback.borrow_mut().on_audio_device_change(self);

    if let Some(b) = self.logger.wrap(WorkerManager::is_installed())
      && b
    {
      self.logger.wrap(self.worker.start());
    }
  }

  pub fn save_storage(&self, storage: &mut dyn Storage) {
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

  fn on_project_change(&self, ctx: &Context, dock_state: Option<&mut DockState<AppTab>>) {
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
    if let Some(dock_state) = dock_state {
      dock_state.retain_tabs(|t| !matches!(t, AppTab::TrackConfig(..)));
    }
  }

  fn show_or_focus_tab(dock_state: &mut DockState<AppTab>, tab: AppTab, as_window: bool) {
    let mut found_tab = None;
    for (path, t) in dock_state.iter_all_tabs() {
      if *t == tab {
        found_tab = Some(path);
        break;
      }
    }

    if let Some(path) = found_tab {
      dock_state.set_active_tab(path).unwrap();
    } else {
      if as_window {
        dock_state.add_window(vec![tab]);
      } else {
        dock_state.push_to_focused_leaf(tab);
      }
    }
  }

  fn on_event(&self, ctx: &Context, event: KsngEvent, dock_state: &mut DockState<AppTab>) {
    match event {
      KsngEvent::ProjectClose => {
        self.project.replace(None);
        self.on_project_change(ctx, Some(dock_state));
      }
      KsngEvent::ProjectNew => {
        self.project.replace(Some(Project::default()));
        self.on_project_change(ctx, Some(dock_state));
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
          self.on_project_change(ctx, Some(dock_state));
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
      KsngEvent::OpenTabWindow(tab) => match tab {
        AppTabInitializer::TrackConfig { track_id } => {
          let project_ref = self.project.borrow();
          let Some(track) = project_ref
            .as_ref()
            .and_then(|p| p.file.tracks.iter().find(|t| t.id == track_id))
          else {
            return;
          };

          let mut active_path = None;
          let mut other_path = None;
          for (path, tab) in dock_state.iter_all_tabs() {
            if let AppTab::TrackConfig(t) = tab
              && t.track_id() == track_id
            {
              active_path = Some(path);
              break;
            } else if matches!(tab, AppTab::TrackConfig(..)) {
              other_path = Some(path);
            }
          }

          if let Some(active_path) = active_path {
            dock_state.set_active_tab(active_path).unwrap();
          } else if let Some(other_path) = other_path
            && let Some(leaf) = dock_state.leaf_mut(other_path.node_path()).ok()
          {
            leaf.append_tab(AppTab::TrackConfig(TrackConfigWindow::new(track)));
          } else {
            dock_state.add_window(vec![AppTab::TrackConfig(TrackConfigWindow::new(track))]);
          }
        }
        AppTabInitializer::Player => Self::show_or_focus_tab(dock_state, AppTab::Player, false),
        AppTabInitializer::LyricsEditor => {
          Self::show_or_focus_tab(dock_state, AppTab::LyricsEditor, false)
        }
        AppTabInitializer::Timeline => Self::show_or_focus_tab(dock_state, AppTab::Timeline, false),
        AppTabInitializer::Preferences => Self::show_or_focus_tab(
          dock_state,
          AppTab::Preferences(PreferencesWindow::new(self.preferences.borrow().clone())),
          true,
        ),
      },
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

  pub fn update(&mut self, ctx: &egui::Context, dock_state: &mut DockState<AppTab>) {
    let mut queue = self.event_queue.borrow_mut();
    while let Some(event) = queue.pop_front() {
      self.on_event(ctx, event, dock_state);
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

  pub fn ui(&self, ui: &mut Ui) {
    let ctx = ui.ctx();

    if self.playback.borrow().state() == PlaybackState::Playing {
      ctx.request_repaint();
    }

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
