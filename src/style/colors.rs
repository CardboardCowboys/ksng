use egui::Color32;
use klib::objects::{event::EventType, track::TrackType};

pub const EVENT_BORDER_COLOR: Color32 = Color32::from_rgb(99, 105, 128);
pub const SELECTED_COLOR: Color32 = Color32::from_rgb(251, 255, 254);
pub const PLAYHEAD_COLOR: Color32 = SELECTED_COLOR;
pub const PLAYHEAD_TOP_COLOR: Color32 = Color32::from_rgb(59, 142, 165);

pub fn color_for_track_type(track_type: TrackType) -> Color32 {
  match track_type {
    TrackType::Lyrics => Color32::from_rgb(150, 76, 166),
    TrackType::Audio => Color32::from_rgb(1, 117, 106),
  }
}

pub fn color_for_event_type(event_type: EventType) -> Color32 {
  match event_type {
    EventType::Lyric => Color32::from_rgb(150, 76, 166),
    EventType::LineBreak => Color32::from_rgb(117, 166, 51),
    EventType::ParagraphBreak => Color32::from_rgb(166, 76, 105),
    EventType::AudioClip => Color32::from_rgb(1, 117, 106),
    EventType::Image => Color32::from_rgb(64, 151, 202),
  }
}

pub fn darkened_color_for_event_type(event_type: EventType) -> Color32 {
  match event_type {
    EventType::AudioClip => Color32::from_rgb(0, 105, 95),
    EventType::Image => Color32::from_rgb(57, 135, 181),
    EventType::LineBreak => Color32::from_rgb(105, 149, 45),
    EventType::Lyric => Color32::from_rgb(135, 68, 149),
    EventType::ParagraphBreak => Color32::from_rgb(149, 68, 94),
  }
}
