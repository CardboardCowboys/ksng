use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default)]
pub struct Timecode(pub u32);
