use std::{
  ops::{Add, Deref, Sub},
  time::Duration,
};

use serde::{Deserialize, Serialize};

/// A timecode represented in milliseconds.
#[derive(Serialize, Deserialize, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timecode(pub u32);

impl Timecode {
  /// Creates a timecode from a floating point seconds value.
  pub fn from_seconds(seconds: f32) -> Self {
    Timecode((seconds * 1000.0) as u32)
  }

  /// Converts the timecode to a floating point seconds value.
  pub fn to_seconds(&self) -> f32 {
    self.0 as f32 / 1000.0
  }

  /// Converts the timecode to a string in the form MM:SS
  pub fn to_string_seconds(&self) -> String {
    let seconds = self.0 / 1000;
    let minutes = seconds / 60;
    let seconds = seconds - (minutes * 60);
    format!("{minutes:02}:{seconds:02}")
  }
}

impl Deref for Timecode {
  type Target = u32;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl Add for Timecode {
  type Output = Timecode;

  fn add(self, rhs: Self) -> Self::Output {
    Timecode(self.0 + rhs.0)
  }
}

impl Sub for Timecode {
  type Output = Timecode;

  fn sub(self, rhs: Self) -> Self::Output {
    Timecode(self.0 - rhs.0)
  }
}

impl From<Timecode> for Duration {
  fn from(value: Timecode) -> Self {
    Duration::from_millis(value.0 as u64)
  }
}

impl From<&Timecode> for Duration {
  fn from(value: &Timecode) -> Self {
    Duration::from_millis(value.0 as u64)
  }
}

impl From<Duration> for Timecode {
  fn from(value: Duration) -> Self {
    Timecode(value.as_millis() as u32)
  }
}
