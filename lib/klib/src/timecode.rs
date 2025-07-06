use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, Clone, Copy)]
pub struct Timecode(pub u32);
