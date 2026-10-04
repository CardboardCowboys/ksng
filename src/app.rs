use egui::Id;
use egui_dock::{DockArea, DockState, NodeIndex, TabViewer};
use uuid::Uuid;

use crate::{
  KsngContext,
  components::{self},
  util::calculate_track_name,
  windows::{preferences::PreferencesWindow, sync::SyncWindow, track_config::TrackConfigWindow},
};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum AppTabInitializer {
  Player,
  LyricsEditor,
  Timeline,
  TrackConfig { track_id: Uuid },
  Preferences,
  Log,
  Sync { track_id: Uuid },
}

#[derive(PartialEq)]
pub enum AppTab {
  Player,
  LyricsEditor,
  Timeline,
  TrackConfig(TrackConfigWindow),
  Preferences(PreferencesWindow),
  Log,
  Sync(SyncWindow),
}

pub struct KsngApp {
  context: KsngContext,
  dock_state: DockState<AppTab>,
}

impl Default for KsngApp {
  fn default() -> Self {
    let mut dock_state = DockState::new(vec![AppTab::LyricsEditor]);
    let [a, _] =
      dock_state
        .main_surface_mut()
        .split_below(NodeIndex::root(), 0.7, vec![AppTab::Timeline]);
    let [_, _] = dock_state
      .main_surface_mut()
      .split_right(a, 0.7, vec![AppTab::Player]);

    Self {
      context: Default::default(),
      dock_state,
    }
  }
}

impl KsngApp {
  /// Called once before the first frame.
  pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
    let app = KsngApp::default();

    if let Some(storage) = cc.storage {
      app.context.load_storage(storage, &cc.egui_ctx);
    }

    egui_extras::install_image_loaders(&cc.egui_ctx);

    app
  }
}

struct AppTabViewer<'a> {
  app: &'a KsngContext,
}

impl<'a> TabViewer for AppTabViewer<'a> {
  type Tab = AppTab;

  fn id(&mut self, tab: &mut Self::Tab) -> Id {
    Id::new(match tab {
      AppTab::Player => "player".to_owned(),
      AppTab::LyricsEditor => "lyrics_editor".to_owned(),
      AppTab::Timeline => "timeline".to_owned(),
      AppTab::TrackConfig(t) => format!("track_config_{}", t.track_id()),
      AppTab::Preferences(_) => "preferences".to_owned(),
      AppTab::Log => "log".to_owned(),
      AppTab::Sync(s) => format!("sync_lyrics_{}", s.track_id()),
    })
  }

  fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
    match tab {
      AppTab::Player => "Player".to_owned(),
      AppTab::LyricsEditor => "Lyrics Editor".to_owned(),
      AppTab::Timeline => "Timeline".to_owned(),
      AppTab::TrackConfig(t) => match &*self.app.project.borrow() {
        Some(p) => format!("Config for {}", calculate_track_name(&p.file, t.track_id())),
        None => "Track Config".to_owned(),
      },
      AppTab::Preferences(_) => "Preferences".to_owned(),
      AppTab::Log => "Log".to_owned(),
      AppTab::Sync(s) => match &*self.app.project.borrow() {
        Some(p) => format!(
          "Lyrics Sync for {}",
          calculate_track_name(&p.file, s.track_id())
        ),
        None => "Lyrics Sync".to_owned(),
      },
    }
    .into()
  }

  fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
    match tab {
      AppTab::Player => components::player::player(self.app, ui),
      AppTab::LyricsEditor => self.app.lyrics_editor.borrow_mut().show(self.app, ui),
      AppTab::Timeline => self.app.timeline.borrow_mut().update(self.app, ui),
      AppTab::TrackConfig(t) => t.show(ui, self.app),
      AppTab::Preferences(p) => p.process(self.app, ui),
      AppTab::Log => components::log::log(self.app, ui),
      AppTab::Sync(s) => s.process(self.app, ui),
    }
  }

  fn on_close(&mut self, tab: &mut Self::Tab) -> egui_dock::tab_viewer::OnCloseResponse {
    if let AppTab::Sync(s) = tab {
      s.on_close(self.app);
    }

    egui_dock::tab_viewer::OnCloseResponse::Close
  }

  fn force_close(&mut self, tab: &mut Self::Tab) -> bool {
    let mut to_close = self.app.tabs_to_close.borrow_mut();
    let ret = match tab {
      AppTab::Player => to_close.take(&AppTabInitializer::Player).is_some(),
      AppTab::LyricsEditor => to_close.take(&AppTabInitializer::LyricsEditor).is_some(),
      AppTab::Timeline => to_close.take(&AppTabInitializer::Timeline).is_some(),
      AppTab::TrackConfig(t) => to_close
        .take(&AppTabInitializer::TrackConfig {
          track_id: t.track_id(),
        })
        .is_some(),
      AppTab::Preferences(_) => to_close.take(&AppTabInitializer::Preferences).is_some(),
      AppTab::Log => to_close.take(&AppTabInitializer::Log).is_some(),
      AppTab::Sync(s) => to_close
        .take(&AppTabInitializer::Sync {
          track_id: s.track_id(),
        })
        .is_some(),
    };

    if ret && let AppTab::Sync(s) = tab {
      s.on_close(self.app);
    }

    ret
  }

  fn scroll_bars(&self, tab: &Self::Tab) -> [bool; 2] {
    match tab {
      AppTab::Timeline => [false, false],
      _ => [true, true],
    }
  }
}

impl eframe::App for KsngApp {
  /// Called by the frame work to save state before shutdown.
  fn save(&mut self, storage: &mut dyn eframe::Storage) {
    self.context.save_storage(storage);
  }

  fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
    self.context.update(ctx, &mut self.dock_state);
  }

  /// Called each time the UI needs repainting, which may be many times per
  /// second.
  fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
    egui::Panel::top("top_panel").show(ui, |ui| {
      components::menu_bar::menu_bar(&self.context, ui);
    });

    DockArea::new(&mut self.dock_state).show_inside(ui, &mut AppTabViewer { app: &self.context });

    components::menu_bar::process_menu_hotkeys(&self.context, ui);
    self.context.ui(ui);
  }
}
