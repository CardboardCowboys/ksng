use egui::Color32;
use klib::objects::track::TrackType;

pub fn color_for_track_type(track_type: TrackType) -> Color32 {
  match track_type {
    TrackType::Lyrics => Color32::from_rgb(150, 76, 166),
    TrackType::Audio => Color32::from_rgb(1, 117, 106),
  }
}
