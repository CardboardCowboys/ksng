use egui::Ui;

use crate::ml::worker::WorkerManager;

#[derive(Default)]
pub struct HtdemucsTab {}

impl HtdemucsTab {
  pub fn htdemucs_tab(&mut self, ui: &mut Ui, worker: &WorkerManager) {}
}
