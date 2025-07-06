#![warn(clippy::all, rust_2018_idioms)]

mod commands;
mod components;
mod error;
mod fs;
mod logger;
mod modals;
mod project;
mod style;
mod ui;
mod ui_event;

mod app;
pub use app::KsngApp;
