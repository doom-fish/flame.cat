use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Frame {
    pub name: String,
    pub start: f64,
    pub end: f64,
    pub depth: u32,
    pub category: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileMetadata {
    pub name: Option<String>,
    pub start_time: f64,
    pub end_time: f64,
    pub format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub metadata: ProfileMetadata,
    pub frames: Vec<Frame>,
}

impl Profile {
    pub fn duration(&self) -> f64 {
        self.metadata.end_time - self.metadata.start_time
    }
}
