use egui::{Label, Response, Sense, Ui, WidgetText};

pub trait KsngUiExt {
  fn inert_label(&mut self, text: impl Into<WidgetText>) -> Response;
  fn inert_heading(&mut self, text: impl Into<WidgetText>) -> Response;
}

impl KsngUiExt for Ui {
  fn inert_label(&mut self, text: impl Into<WidgetText>) -> Response {
    let label = Label::new(text).sense(Sense::empty());
    let (pos, galley, res) = label.layout_in_ui(self);

    self
      .painter()
      .galley(pos, galley, self.style().visuals.text_color());
    res
  }

  fn inert_heading(&mut self, text: impl Into<WidgetText>) -> Response {
    self.inert_label(text.into().heading())
  }
}
