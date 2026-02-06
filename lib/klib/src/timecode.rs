use std::{
  ops::{Add, Deref, Div, Mul, Sub},
  time::Duration,
};

use serde::{Deserialize, Serialize};

/// A timecode represented in milliseconds.
#[derive(Serialize, Deserialize, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timecode(pub u32);

impl Timecode {
  /// The minimum timecode value.
  pub const MIN: Timecode = Timecode(0);
  /// The maximum timecode value.
  pub const MAX: Timecode = Timecode(u32::MAX);

  /// Creates a timecode from a floating point seconds value.
  pub fn from_seconds(seconds: f32) -> Self {
    Timecode((seconds * 1000.0) as u32)
  }

  /// Creates a timecode from a floating point seconds value.
  pub fn from_seconds_f64(seconds: f64) -> Self {
    Timecode((seconds * 1000.0) as u32)
  }

  /// Converts the timecode to a floating point seconds value.
  pub fn to_seconds(&self) -> f32 {
    self.0 as f32 / 1000.0
  }

  /// Converts the timecode to a floating point seconds value.
  pub fn to_seconds_f64(&self) -> f64 {
    self.0 as f64 / 1000.0
  }

  /// Converts the timecode to a string in the form MM:SS
  pub fn to_string_seconds(&self) -> String {
    let seconds = self.0 / 1000;
    let minutes = seconds / 60;
    let seconds = seconds - (minutes * 60);
    format!("{minutes:02}:{seconds:02}")
  }

  /// Converts the timecode to a string in the form MM:SS.MMM
  pub fn to_string_seconds_frac(&self) -> String {
    let seconds = self.0 / 1000;
    let minutes = seconds / 60;
    let frac = self.0 - (seconds * 1000);
    let seconds = seconds - (minutes * 60);
    format!("{minutes:02}:{seconds:02}.{frac:03}")
  }

  /// Compares two (start, end) pairs and returns true if they overlap.
  pub fn ranges_overlap(a: (Timecode, Timecode), b: (Timecode, Timecode)) -> bool {
    let (a_start, a_end) = a;
    let (b_start, b_end) = b;
    (a_start >= b_start && a_start < b_end)
      || (a_end >= b_start && a_end < b_end)
      || (b_start >= a_start && b_start < a_end)
      || (b_end >= a_start && b_end < a_end)
  }

  /// Returns the minimum of `self` and `rhs`.
  pub fn min(&self, rhs: Self) -> Timecode {
    Timecode(self.0.min(rhs.0))
  }

  /// Returns the maximum of `self` and `rhs`.
  pub fn max(&self, rhs: Self) -> Timecode {
    Timecode(self.0.max(rhs.0))
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
    Timecode(self.0.saturating_sub(rhs.0))
  }
}

impl Mul for Timecode {
  type Output = Timecode;

  fn mul(self, rhs: Self) -> Self::Output {
    Timecode(self.0 * rhs.0)
  }
}

impl Div for Timecode {
  type Output = Timecode;

  fn div(self, rhs: Self) -> Self::Output {
    Timecode(self.0 / rhs.0)
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
