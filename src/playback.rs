use klib::timecode::Timecode;

use crate::{audio::mixer::AudioMixer, KsngApp};

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
  #[default]
  Stopped,
  Playing,
}

#[derive(Default)]
pub struct Playback {
  state: PlaybackState,
  mixer: AudioMixer,
}

impl Playback {
  pub fn on_audio_change(&mut self, app: &KsngApp) {
    if let Some(project) = app.project.borrow().as_ref() {
      self.mixer.update_streams(project);
    } else {
      self.mixer.reset();
      self.state = PlaybackState::Stopped;
    }
  }

  pub fn state(&self) -> PlaybackState {
    self.state
  }

  pub fn position(&self) -> Timecode {
    self.mixer.position()
  }

  pub fn toggle_state(&mut self) {
    self.update_state(if self.state == PlaybackState::Playing {
      PlaybackState::Stopped
    } else {
      PlaybackState::Playing
    });
  }

  pub fn update_state(&mut self, new_state: PlaybackState) {
    match new_state {
      PlaybackState::Stopped => self.mixer.pause(),
      PlaybackState::Playing => self.mixer.play(),
    }

    self.state = new_state;
  }

  pub fn seek(&self, time: Timecode) {
    self.mixer.seek(time);
  }
}
