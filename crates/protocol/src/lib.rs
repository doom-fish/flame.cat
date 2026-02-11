pub mod commands;
pub mod shared_str;
pub mod theme;
pub mod types;
pub mod visual_profile;

pub use commands::{RenderCommand, TextAlign};
pub use shared_str::SharedStr;
pub use theme::ThemeToken;
pub use types::{ClockKind, Color, Point, Rect, TimeDomain};
pub use visual_profile::{
    AsyncSpan, CounterSample, CounterTrack, CounterUnit, CpuNode, CpuSamples, FlowArrow,
    FrameTiming, InstantEvent, Marker, MarkerScope, NetworkRequest, ObjectEvent, ObjectPhase,
    ProfileMeta, Screenshot, SourceFormat, Span, SpanCategory, SpanKind, ThreadGroup, ValueUnit,
    VisualProfile,
};

/// Viewport describing the visible region — passed to view transforms so
/// they can cull off-screen frames and compute pixel-space coordinates.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Viewport {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// Device pixel ratio (1.0, 2.0, 3.0 …)
    pub dpr: f64,
}
