use egui::{Checkbox, ComboBox, DragValue, Slider, Ui};
use klib::{
  style::{Color32, FontManager, FontStyle},
  timecode::Timecode,
  util::editable_config::{EditableConfig, EditableConfigUi},
};
use regex::Regex;

const FONT_WEIGHT_NAMES: &[&str] = &[
  "Invisible",
  "Thin",
  "Extra Light",
  "Light",
  "Normal",
  "Medium",
  "Semi Bold",
  "Bold",
  "Extra Bold",
  "Black",
  "Extra Black",
];

const FONT_WIDTH_NAMES: &[&str] = &[
  "Ultra Condensed",
  "Extra Condensed",
  "Condensed",
  "Semi Condensed",
  "Normal",
  "Semi Expanded",
  "Expanded",
  "Extra Expanded",
  "Ultra Expanded",
];

static TIMECODE_REGEX: once_cell::sync::Lazy<Regex> = once_cell::sync::Lazy::new(|| {
  Regex::new(r"((?<minutes>\d+)\:)?(?<seconds>\d+)(\.(?<frac>\d+))?").unwrap()
});

struct EguiEditableConfigUi<'ui> {
  ui: &'ui mut Ui,
  id: String,
  font_mgr: FontManager,
}

impl<'ui> EguiEditableConfigUi<'ui> {
  pub fn new(ui: &'ui mut Ui, id: String) -> EguiEditableConfigUi<'ui> {
    EguiEditableConfigUi {
      ui,
      id,
      font_mgr: FontManager::default(),
    }
  }
}

fn config_row<F: FnOnce(&mut Ui)>(key: &str, ui: &mut Ui, f: F) {
  if !key.is_empty() {
    ui.label(key);
  }
  f(ui);
  ui.end_row();
}

impl<'ui> EditableConfigUi for EguiEditableConfigUi<'ui> {
  fn checkbox(&mut self, key: &str, changed: &mut bool, value: bool) -> bool {
    let mut new_value = value;
    config_row(key, self.ui, |ui| {
      ui.add(Checkbox::without_text(&mut new_value));
    });
    if new_value != value {
      *changed = true;
    }
    new_value
  }

  fn slider(&mut self, key: &str, changed: &mut bool, min: f32, max: f32, value: f32) -> f32 {
    let mut new_value = value;
    config_row(key, self.ui, |ui| {
      ui.add(Slider::new(&mut new_value, min..=max));
    });
    if new_value != value {
      *changed = true;
    }
    new_value
  }

  fn float(
    &mut self,
    key: &str,
    changed: &mut bool,
    min: Option<f32>,
    max: Option<f32>,
    value: f32,
  ) -> f32 {
    let mut new_value = value;
    config_row(key, self.ui, |ui| {
      let mut drag_val = DragValue::new(&mut new_value).min_decimals(1);
      if let Some(min) = min {
        if let Some(max) = max {
          drag_val = drag_val.range(min..=max);
        } else {
          drag_val = drag_val.range(min..=f32::MAX);
        }
      }
      ui.add(drag_val);
    });
    if new_value != value {
      *changed = true;
    }
    new_value
  }

  fn integer(
    &mut self,
    key: &str,
    changed: &mut bool,
    min: Option<i64>,
    max: Option<i64>,
    value: i64,
  ) -> i64 {
    let mut new_value = value;
    config_row(key, self.ui, |ui| {
      let mut drag_val = DragValue::new(&mut new_value);
      if let Some(min) = min {
        if let Some(max) = max {
          drag_val = drag_val.range(min..=max);
        } else {
          drag_val = drag_val.range(min..=i64::MAX);
        }
      }
      ui.add(drag_val);
    });
    if new_value != value {
      *changed = true;
    }
    new_value
  }

  fn normalized_rect(&mut self, key: &str, changed: &mut bool, value: klib::Rect) -> klib::Rect {
    let mut new_value = value;
    // TODO: better rect editor
    config_row(key, self.ui, |ui| {
      ui.label("X0");
      ui.add(DragValue::new(&mut new_value.x0).range(0.0..=1.0));
      ui.label("Y0");
      ui.add(DragValue::new(&mut new_value.y0).range(0.0..=1.0));
      ui.label("X1");
      ui.add(DragValue::new(&mut new_value.x1).range(0.0..=1.0));
      ui.label("Y1");
      ui.add(DragValue::new(&mut new_value.y1).range(0.0..=1.0));
    });
    new_value = new_value.conform();
    if new_value != value {
      *changed = true;
    }
    new_value
  }

  fn config(&mut self, key: &str, changed: &mut bool, value: &mut dyn EditableConfig) {
    config_row(key, self.ui, |ui| {
      let id = format!("{}_{}", self.id, key);
      if !key.is_empty() {
        if config_editor(ui, id, value) {
          *changed = true;
        }
      } else {
        let mut new_ui_harness = EguiEditableConfigUi::new(ui, id);
        if value.edit(&mut new_ui_harness) {
          *changed = true;
        }
      }
    });
  }

  fn dropdown<'a>(
    &mut self,
    key: &str,
    changed: &mut bool,
    options: &[&'a str],
    value: &'a str,
  ) -> &'a str {
    let mut new_idx = options.iter().position(|v| *v == value).unwrap_or(0);
    config_row(key, self.ui, |ui| {
      ComboBox::new(format!("{}_{}", self.id, key), "")
        .selected_text(value)
        .show_ui(ui, |ui| {
          for (i, option) in options.iter().enumerate() {
            ui.selectable_value(&mut new_idx, i, *option);
          }
        });
    });
    let new_value = options[new_idx];
    if new_value != value {
      *changed = true;
    }
    new_value
  }

  fn timecode(
    &mut self,
    key: &str,
    changed: &mut bool,
    value: klib::timecode::Timecode,
  ) -> klib::timecode::Timecode {
    let mut new_value = value.0;
    config_row(key, self.ui, |ui| {
      let drag_val = DragValue::new(&mut new_value)
        .range(0..=u32::MAX)
        .custom_formatter(|n, _| Timecode(n as u32).to_string_seconds_frac())
        .custom_parser(|s| {
          let captures = TIMECODE_REGEX.captures(s)?;
          let minutes = &captures
            .name("minutes")
            .iter()
            .flat_map(|m| m.as_str().parse::<u32>().ok())
            .next()
            .unwrap_or(0);
          let seconds = &captures
            .name("seconds")
            .iter()
            .flat_map(|m| m.as_str().parse::<u32>().ok())
            .next()
            .unwrap_or(0);
          let frac = &captures
            .name("frac")
            .iter()
            .flat_map(|m| m.as_str().parse::<u32>().ok())
            .next()
            .unwrap_or(0);
          Some((frac + (seconds * 1000) + (minutes * 60 * 1000)) as f64)
        });
      ui.add(drag_val);
    });
    let new_value = Timecode(new_value);
    if new_value != value {
      *changed = true;
    }
    new_value
  }

  fn font(&mut self, key: &str, changed: &mut bool, value: klib::style::Font) -> klib::style::Font {
    let mut new_value = value.clone();
    config_row(key, self.ui, |ui| {
      ui.vertical(|ui| {
        let prev_family_name = new_value.family.clone();
        ComboBox::new(format!("{}_{}_family", self.id, key), "Family")
          .selected_text(&new_value.family)
          .show_ui(ui, |ui| {
            for value in self.font_mgr.font_names() {
              ui.selectable_value(&mut new_value.family, value.clone(), value);
            }
          });
        if new_value.family != prev_family_name {
          *changed = true;
        }
        let info = self.font_mgr.font_info(&new_value.family);
        let prev_weight = new_value.weight.0;
        ComboBox::new(format!("{}_{}_weight", self.id, key), "Weight")
          .selected_text(FONT_WEIGHT_NAMES[(new_value.weight.0 / 100) as usize])
          .show_ui(ui, |ui| {
            for value in info.weights {
              ui.selectable_value(
                &mut new_value.weight.0,
                value,
                FONT_WEIGHT_NAMES[(value / 100) as usize],
              );
            }
          });
        if prev_weight != new_value.weight.0 {
          *changed = true;
        }
        let prev_width = new_value.width.0;
        ComboBox::new(format!("{}_{}_width", self.id, key), "Width")
          .selected_text(FONT_WIDTH_NAMES[(new_value.width.0 - 1) as usize])
          .show_ui(ui, |ui| {
            for value in info.widths {
              ui.selectable_value(
                &mut new_value.width.0,
                value,
                FONT_WIDTH_NAMES[(value - 1) as usize],
              );
            }
          });
        if prev_width != new_value.width.0 {
          *changed = true;
        }
        let prev_style = new_value.style;
        ComboBox::new(format!("{}_{}_style", self.id, key), "Style")
          .selected_text(format!("{:?}", new_value.style))
          .show_ui(ui, |ui| {
            ui.selectable_value(&mut new_value.style, FontStyle::Normal, "Normal");
            ui.selectable_value(&mut new_value.style, FontStyle::Italic, "Italic");
            ui.selectable_value(&mut new_value.style, FontStyle::Oblique, "Oblique");
          });
        if prev_style != new_value.style {
          *changed = true;
        }
      });
    });

    new_value
  }

  fn color(
    &mut self,
    key: &str,
    changed: &mut bool,
    value: klib::style::Color32,
  ) -> klib::style::Color32 {
    let bytes = value.to_bytes();
    let mut new_value =
      egui::Color32::from_rgba_unmultiplied(bytes[0], bytes[1], bytes[2], bytes[3]);
    config_row(key, self.ui, |ui| {
      ui.color_edit_button_srgba(&mut new_value);
    });
    let new_color = Color32::from_rgba(new_value.r(), new_value.g(), new_value.b(), new_value.a());
    if new_color != value {
      *changed = true;
    }
    new_color
  }
}

pub fn config_editor(ui: &mut Ui, id: String, config_obj: &mut dyn EditableConfig) -> bool {
  let mut changed = false;
  egui::Grid::new(&id)
    .num_columns(2)
    .spacing([40.0, 2.0])
    .striped(true)
    .show(ui, |ui| {
      let mut ui = EguiEditableConfigUi::new(ui, format!("{}_editor", id));
      changed = config_obj.edit(&mut ui);
    });
  changed
}
