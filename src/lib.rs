#![warn(clippy::all, rust_2018_idioms)]

mod audio;
mod commands;
mod components;
mod fs;
mod modals;
mod playback;
mod preferences;
mod project;
mod selection;
mod style;
mod util;
mod video;
mod windows;

mod app;
pub use app::KsngApp;
