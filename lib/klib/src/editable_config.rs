use crate::style::{Color32, Font};

pub trait EditableEnum {
  fn items() -> &'static [&'static str];
  fn selected_item(&self) -> &'static str;
  fn set_item(&mut self, item: &str);
}

pub enum EditableConfigValue<'a> {
  Color(&'a mut Color32),
  FontFamily(&'a mut String),
  Float(&'a mut f32),
  Integer(&'a mut i32),
}

pub struct EditableConfigItem<'a> {
  pub value: EditableConfigValue<'a>,
}

pub trait EditableConfig {
  fn items<'a>(&'a mut self) -> Vec<EditableConfigItem<'a>>;
}
