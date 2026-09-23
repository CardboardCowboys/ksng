use egui::Id;
use egui_dock::{DockArea, DockState, NodeIndex, TabViewer};

use crate::{
  KsngContext,
  components::{self},
};

#[derive(Debug, PartialEq)]
pub enum AppTab {
  Player,
  LyricsEditor,
  Timeline,
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
    Id::new(format!("{tab:?}"))
  }

  fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
    match tab {
      AppTab::Player => "Player",
      AppTab::LyricsEditor => "Lyrics Editor",
      AppTab::Timeline => "Timeline",
    }
    .into()
  }

  fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
    match tab {
      AppTab::Player => components::player::player(self.app, ui),
      AppTab::LyricsEditor => self.app.lyrics_editor.borrow_mut().show(self.app, ui),
      AppTab::Timeline => self.app.timeline.borrow_mut().update(self.app, ui),
    }
  }
}

impl eframe::App for KsngApp {
  /// Called by the frame work to save state before shutdown.
  fn save(&mut self, storage: &mut dyn eframe::Storage) {
    self.context.save_storage(storage);
  }

  fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
    self.context.update(ctx);
  }

  /// Called each time the UI needs repainting, which may be many times per
  /// second.
  fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
    egui::Panel::top("top_panel").show(ui, |ui| {
      components::menu_bar::menu_bar(&mut self.dock_state, &self.context, ui);
    });

    DockArea::new(&mut self.dock_state).show_inside(ui, &mut AppTabViewer { app: &self.context });

    self.context.ui(ui);
  }
}
