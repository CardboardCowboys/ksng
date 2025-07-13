use egui::{Label, Response, Sense, Ui, WidgetText};

pub fn inert_label(ui: &mut Ui, text: impl Into<WidgetText>) -> Response {
  let label = Label::new(text).sense(Sense::empty());
  let (pos, galley, res) = label.layout_in_ui(ui);

  ui.painter()
    .galley(pos, galley, ui.style().visuals.text_color());
  res
}
