#![feature(read_le)]
#![warn(clippy::all, rust_2018_idioms)]

mod audio;
mod commands;
mod components;
mod context;
mod fs;
mod locker;
mod ml;
mod modals;
mod playback;
mod preferences;
mod project;
mod selection;
mod style;
mod tabs;
mod util;
mod video;

mod app;
pub use app::KsngApp;
pub use context::KsngContext;
pub use util::logger::Logger;
