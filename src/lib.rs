#![warn(clippy::all, rust_2018_idioms)]

mod audio;
mod commands;
mod components;
mod fs;
mod modals;
mod playback;
mod project;
mod selection;
mod style;
mod util;

mod app;
pub use app::KsngApp;
