#![warn(clippy::all, rust_2018_idioms)]

mod commands;
mod error;
mod fs;
mod icons;
mod logger;
mod modals;
mod project;
mod ui;
mod ui_event;

mod app;
pub use app::KsngApp;
