use klib::timecode::Timecode;

use crate::{
  audio::{config::AudioConfig, mixer::AudioMixer},
  util::logger::Logger,
  KsngApp,
};

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
  #[default]
  Stopped,
  Playing,
}

pub struct Playback {
  state: PlaybackState,
  mixer: AudioMixer,
  logger: Logger,
}

impl Playback {
  pub fn new(config: &AudioConfig, logger: Logger) -> Playback {
    Playback {
      state: PlaybackState::Stopped,
      mixer: AudioMixer::new(config, logger.clone()).unwrap(),
      logger,
    }
  }

  pub fn on_audio_change(&mut self, app: &KsngApp) {
    if let Some(project) = app.project.borrow().as_ref() {
      self.logger.wrap(self.mixer.update_streams(project));
    } else {
      self.mixer.reset();
      self.state = PlaybackState::Stopped;
    }
  }

  pub fn on_audio_device_change(&mut self, app: &KsngApp) {
    let pos = self.mixer.position();
    self.logger.wrap(
      self
        .mixer
        .update_audio_device(&app.preferences.borrow().audio_config),
    );
    self.mixer.seek(pos);
    self.state = PlaybackState::Stopped;
    self.on_audio_change(app);
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
