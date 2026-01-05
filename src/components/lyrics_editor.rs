use std::fmt::{Display, Write};

use egui::{
  Color32, FontId, MenuBar, TextFormat, Ui,
  text::{LayoutJob, LayoutSection},
};
use klib::{
  objects::{
    event::{Event, EventType, EventValue},
    track::TrackType,
  },
  timecode::Timecode,
};
use uuid::Uuid;

use crate::{KsngApp, project::Project};

const ESCAPE_CHAR: char = '\\';
const SYLLABLE_SEPARATOR: char = '-';

struct LyricsEditorText {
  elements: Vec<LyricsEditorTextElement>,
  text: String,
}

impl LyricsEditorText {
  pub fn new(elements: Vec<LyricsEditorTextElement>) -> Self {
    let mut s = String::new();
    let mut last_elem_lyric = false;
    for e in &elements {
      if last_elem_lyric && e.event_type == EventType::Lyric {
        let _ = write!(&mut s, " ");
      }
      let _ = write!(&mut s, "{}", e);
      last_elem_lyric = e.event_type == EventType::Lyric;
    }

    LyricsEditorText { elements, text: s }
  }
}

#[derive(Default)]
struct LyricsData {
  tracks: Vec<LyricsEditorText>,
  selected_lyrics_track: usize,
}

#[derive(Default)]
pub struct LyricsEditor {
  current_data: Option<LyricsData>,
}

impl LyricsEditor {
  pub fn on_project_change(&mut self, app: &KsngApp) {
    self.on_lyrics_change(app);
    if let Some(data) = &mut self.current_data {
      data.selected_lyrics_track = 0;
    }
  }

  pub fn on_lyrics_change(&mut self, app: &KsngApp) {
    match app.project.borrow().as_ref() {
      Some(project) => self.rebuild_lyrics(project),
      None => self.current_data = None,
    };
  }

  fn rebuild_lyrics(&mut self, project: &Project) {
    if self.current_data.is_none() {
      self.current_data.replace(LyricsData::default());
    }
    let Some(data) = &mut self.current_data else {
      return;
    };

    data.tracks.clear();

    for track in &project.file.tracks {
      if track.track_type != TrackType::Lyrics {
        continue;
      }

      let mut elements = Vec::new();
      let mut current_events = Vec::new();
      let mut last_lyric_id = Uuid::default();

      for ev in track.events.iter() {
        if ev.event_type == EventType::Lyric
          && let Some(linked_id) = ev.linked_id
          && linked_id == last_lyric_id
        {
          current_events.push(ev.clone());
          last_lyric_id = ev.id;
          continue;
        }

        if !current_events.is_empty() {
          elements.push(LyricsEditorTextElement::new(
            EventType::Lyric,
            current_events,
          ));
          current_events = Vec::new();
          last_lyric_id = Uuid::default();
        }

        if ev.event_type == EventType::Lyric {
          current_events.push(ev.clone());
          last_lyric_id = ev.id;
        } else {
          elements.push(LyricsEditorTextElement::new(
            ev.event_type,
            vec![ev.clone()],
          ));
        }
      }

      if !current_events.is_empty() {
        elements.push(LyricsEditorTextElement::new(
          EventType::Lyric,
          current_events,
        ));
      }

      data.tracks.push(LyricsEditorText::new(elements));
    }
  }

  pub fn show(&mut self, app: &KsngApp, ui: &mut Ui) {
    egui::TopBottomPanel::top("lyrics_editor#top").show_inside(ui, |ui| {
      MenuBar::new().ui(ui, |ui| {
        // If there aren't any tracks available, we just want to show "No lyrics tracks"
        let has_lyrics_tracks = self
          .current_data
          .as_ref()
          .map(|d| !d.tracks.is_empty())
          .unwrap_or_default();
        ui.add_enabled_ui(has_lyrics_tracks, |ui| match &mut self.current_data {
          Some(data) => {
            let current_track_name = format!("Lyrics Track #{}", data.selected_lyrics_track + 1);
            let mut selected_lyrics_track = data.selected_lyrics_track;
            ui.menu_button(current_track_name, |ui| {
              for i in 0..data.tracks.len() {
                if ui.button(format!("Lyrics Track #{}", i + 1)).clicked() {
                  selected_lyrics_track = i;
                }
              }
            });
            data.selected_lyrics_track = selected_lyrics_track;
          }
          None => {
            ui.menu_button("No lyrics tracks", |_ui| {});
          }
        });
      });
    });

    egui::CentralPanel::default().show_inside(ui, |ui| {
      egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
        let Some(data) = &self.current_data else {
          return;
        };

        let job = self.layout_lyrics(data, ui);
        ui.add(egui::Label::new(job).selectable(true));
      });
    });
  }

  fn layout_lyrics(&self, data: &LyricsData, ui: &mut Ui) -> LayoutJob {
    let track = &data.tracks[data.selected_lyrics_track];

    let mut job = LayoutJob {
      text: track.text.clone(),
      ..Default::default()
    };

    // TODO: highlight text based on playback position
    job.sections.push(LayoutSection {
      leading_space: 0.0,
      byte_range: 0..job.text.len(),
      format: TextFormat {
        font_id: FontId::proportional(20.0),
        color: Color32::WHITE,
        italics: false,
        ..Default::default()
      },
    });

    job
  }
}

struct LyricsEditorTextElement {
  event_type: EventType,
  start_timecode: Timecode,
  end_timecode: Timecode,
  events: Vec<Event>,
}

impl LyricsEditorTextElement {
  pub fn new(event_type: EventType, events: Vec<Event>) -> Self {
    let start_timecode = events
      .iter()
      .map(|e| e.start_timecode)
      .min()
      .unwrap_or_default();
    let end_timecode = events
      .iter()
      .map(|e| e.end_timecode)
      .max()
      .unwrap_or_default();
    LyricsEditorTextElement {
      event_type,
      start_timecode,
      end_timecode,
      events,
    }
  }
}

impl Display for LyricsEditorTextElement {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self.event_type {
      EventType::LineBreak => f.write_char('\n')?,
      EventType::ParagraphBreak => f.write_str("\n\n")?,
      EventType::Lyric => {
        let mut first = true;
        for e in &self.events {
          let Some(EventValue::Lyric { text }) = &e.value else {
            continue;
          };

          if !first {
            f.write_char(SYLLABLE_SEPARATOR)?;
          } else {
            first = false;
          }

          for ch in text.chars() {
            if ch == SYLLABLE_SEPARATOR {
              f.write_char(ESCAPE_CHAR)?;
            }

            f.write_char(ch);
          }
        }
      }
      _ => {}
    }

    Ok(())
  }
}
