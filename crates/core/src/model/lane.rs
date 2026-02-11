use serde::{Deserialize, Serialize};

/// Which visualization mode a lane is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewType {
    TimeOrder,
    LeftHeavy,
    Sandwich,
    ComponentTree,
    Ranked,
    CommitTimeline,
}

/// A lane is the fundamental layout primitive — a horizontal strip
/// displaying one view of one data source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lane {
    pub id: String,
    pub view_type: ViewType,
    /// Height in logical pixels.
    pub height: f64,
    /// Vertical scroll offset within the lane.
    pub scroll_y: f64,
    /// The profile this lane is displaying.
    pub profile_index: usize,
    /// For Sandwich view: the selected frame id.
    pub selected_frame: Option<u64>,
}
