#![warn(clippy::all, rust_2018_idioms)]

mod async_handler;
mod data;
mod error;
mod icons;
mod logger;
mod modals;
mod project;
mod ui;
mod ui_event;

mod app;
pub use app::KsngApp;
