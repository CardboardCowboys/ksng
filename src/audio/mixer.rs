use std::{
  collections::{hash_map::Entry, HashMap, HashSet},
  time::Duration,
};

use klib::{
  objects::{
    audio::{AudioFile, AudioFileSource, AudioFileType},
    event::{EventType, EventValue},
    track::{AudioTrackValue, TrackType, TrackValue},
  },
  timecode::Timecode,
};
use log::info;
use rodio::{mixer::mixer, source::Zero, Decoder, OutputStream, OutputStreamBuilder, Sink, Source};

use crate::project::Project;

pub struct AudioMixer {
  sink: Sink,
  // We need to store the output stream so it's not dropped and our output cancelled.
  #[allow(dead_code)]
  output_stream: OutputStream,
}

impl Default for AudioMixer {
  fn default() -> Self {
    let builder = OutputStreamBuilder::from_default_device().expect("Can't open output device");
    info!("Found output stream: {builder:?}");
    let output = builder.open_stream().expect("Can't open output stream");
    let sink = Sink::connect_new(output.mixer());
    sink.pause();
    Self {
      sink,
      output_stream: output,
    }
  }
}

impl AudioMixer {
  pub fn play(&self) {
    self.sink.play();
  }

  pub fn pause(&self) {
    self.sink.pause();
  }

  pub fn position(&self) -> Timecode {
    self.sink.get_pos().into()
  }

  pub fn reset(&mut self) {
    self.sink.stop();
    self.sink.clear();
  }

  pub fn update_streams(&mut self, project: &Project) {
    let (mixer, mixer_source) = mixer(2, 48000);

    for track in project
      .file
      .tracks
      .iter()
      .filter(|t| t.track_type == TrackType::Audio)
    {
      let track_value = if let Some(TrackValue::Audio(audio)) = &track.track_value {
        audio.clone()
      } else {
        AudioTrackValue::default()
      };

      if track_value.muted {
        continue;
      }

      for event in track
        .events
        .iter()
        .filter(|e| e.event_type == EventType::AudioClip)
      {
        if let Some(EventValue::AudioClip { offset, file }) = &event.value {
          if let Some(decoder) = Self::create_decoder(file) {
            let source = decoder
              .skip_duration(offset.into())
              .take_duration((event.end_timecode - event.start_timecode).into())
              .delay(event.start_timecode.into())
              .amplify_normalized(track_value.volume);
            mixer.add(source);
          }
        }
      }
    }

    self.sink.append(mixer_source);
    // Add Zero source so we can continue playing after the audio has ended.
    self.sink.append(Zero::new(2, 48000))
  }

  fn create_decoder(file: &AudioFile) -> Option<Decoder<std::fs::File>> {
    let source_path = if let AudioFileSource::Path(path) = &file.source {
      Some(path.as_path())
    } else {
      None
    }?;

    let file_handle = std::fs::File::open(source_path).ok()?;
    let len = file_handle.metadata().ok()?.len();

    let hint = match file.file_type {
      AudioFileType::Mp3 => "mp3",
      AudioFileType::Wave => "wav",
      AudioFileType::Flac => "flac",
      AudioFileType::Aac => "symphonia",
      AudioFileType::Ogg => "vorbis",
    };

    Decoder::builder()
      .with_data(file_handle)
      .with_byte_len(len)
      .with_seekable(true)
      .with_hint(hint)
      .with_gapless(true)
      .build()
      .ok()
  }
}
