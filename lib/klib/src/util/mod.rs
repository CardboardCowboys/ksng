use harfbuzz_rs::{Face, Font};

pub mod easing;
pub mod editable_config;
pub mod rect;

pub fn skfont_to_harfbuzz_font(skfont: &skia_safe::Font) -> harfbuzz_rs::Owned<Font<'_>> {
  let face = Face::from_table_func(|tag| {
    let mut table_data = Vec::new();
    let size = skfont.typeface().get_table_size(tag.0)?;
    table_data.resize(size, 0);
    skfont.typeface().get_table_data(tag.0, &mut table_data);
    Some(table_data.into())
  });

  Font::new(face)
}
