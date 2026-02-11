use serde::{Deserialize, Serialize};

/// A single stack frame span in the profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Frame {
    /// Unique identifier within this profile.
    pub id: u64,
    /// Display name (function, component, etc.).
    pub name: String,
    /// Start time in microseconds from profile start.
    pub start: f64,
    /// End time in microseconds from profile start.
    pub end: f64,
    /// Stack depth (0 = top-level).
    pub depth: u32,
    /// Optional category for grouping / coloring.
    pub category: Option<String>,
    /// Index of the parent frame, if any.
    pub parent: Option<u64>,
    /// Self time (exclusive of children).
    pub self_time: f64,
}

impl Frame {
    pub fn duration(&self) -> f64 {
        self.end - self.start
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileMetadata {
    pub name: Option<String>,
    pub start_time: f64,
    pub end_time: f64,
    /// Source format identifier (e.g. "chrome", "firefox", "react").
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

    /// Get a frame by its id.
    pub fn frame(&self, id: u64) -> Option<&Frame> {
        self.frames.iter().find(|f| f.id == id)
    }

    /// Direct children of the given frame (or top-level frames if `None`).
    pub fn children(&self, parent: Option<u64>) -> Vec<&Frame> {
        self.frames.iter().filter(|f| f.parent == parent).collect()
    }
}
