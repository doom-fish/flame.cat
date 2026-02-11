pub mod commands;
pub mod theme;
pub mod types;

pub use commands::{RenderCommand, TextAlign};
pub use theme::ThemeToken;
pub use types::{Color, Point, Rect};

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
