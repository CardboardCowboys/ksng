use chrono::Timelike;
use egui::{Color32, FontId, ScrollArea, TextFormat, Ui, Vec2b, text::LayoutJob};

use crate::{KsngContext, util::logger::LogType};

fn log_layout(app: &KsngContext, ui: &mut Ui) {
  let font = FontId::monospace(12.0);
  let text_col = ui.style().visuals.text_color();
  let weak_col = ui.style().visuals.weak_text_color();

  let mut job = LayoutJob::default();
  let messages = app.logger.messages.read().unwrap();
  for msg in &*messages {
    log::info!("{}", msg.text);
    let date = format!(
      "[{:02}:{:02}:{:02}] ",
      msg.time.hour(),
      msg.time.minute(),
      msg.time.second()
    );
    job.append(&date, 0.0, TextFormat::simple(font.clone(), weak_col));
    let color = match msg.log_type {
      LogType::Debug => Color32::CYAN,
      LogType::Info => text_col,
      LogType::Warning => Color32::YELLOW,
      LogType::Error => Color32::RED,
    };
    job.append(&msg.text, 0.0, TextFormat::simple(font.clone(), color));
    job.append("\n", 0.0, TextFormat::simple(font.clone(), color));
  }

  ui.label(job);
}

pub fn log(app: &KsngContext, ui: &mut Ui) {
  ScrollArea::new(Vec2b::new(true, true)).show(ui, |ui| {
    log_layout(app, ui);
  });
}
