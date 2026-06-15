#![warn(clippy::all, rust_2018_idioms)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

fn main() -> eframe::Result {
  colog::init();

  let native_options = eframe::NativeOptions {
    viewport: egui::ViewportBuilder::default()
      .with_inner_size([1600.0, 900.0])
      .with_min_inner_size([300.0, 220.0])
      .with_icon(
        // NOTE: Adding an icon is optional
        eframe::icon_data::from_png_bytes(&include_bytes!("../assets/ksng-icon.png")[..])
          .expect("Failed to load icon"),
      ),
    ..Default::default()
  };
  eframe::run_native(
    &format!("ksng {}", env!("CARGO_PKG_VERSION")),
    native_options,
    Box::new(|cc| Ok(Box::new(ksng::KsngApp::new(cc)))),
  )
}
