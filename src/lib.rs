#![warn(clippy::all, rust_2018_idioms)]

mod audio;
mod commands;
mod components;
mod error;
mod fs;
mod logger;
mod modals;
mod project;
mod selection;
mod style;
mod ui;
mod ui_event;

mod app;
pub use app::KsngApp;
