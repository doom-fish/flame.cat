use serde::{Deserialize, Serialize};

use crate::shared_str::SharedStr;
use crate::theme::ThemeToken;
use crate::types::{Point, Rect};

/// A single, stateless render instruction.
///
/// The core emits a `Vec<RenderCommand>` for each view. Renderers consume
/// this list sequentially — each command carries all the data it needs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RenderCommand {
    /// Draw a filled rectangle, optionally with a text label and a logical
    /// frame identifier (for hit-testing / selection).
    DrawRect {
        rect: Rect,
        color: ThemeToken,
        border_color: Option<ThemeToken>,
        label: Option<SharedStr>,
        frame_id: Option<u64>,
    },

    /// Draw a text string at a position.
    DrawText {
        position: Point,
        text: SharedStr,
        color: ThemeToken,
        font_size: f64,
        align: TextAlign,
    },

    /// Draw a line segment.
    DrawLine {
        from: Point,
        to: Point,
        color: ThemeToken,
        width: f64,
    },

    /// Restrict subsequent drawing to a rectangular region.
    SetClip { rect: Rect },

    /// Remove the active clip region.
    ClearClip,

    /// Push an affine transform (applied to all subsequent commands until
    /// the matching `PopTransform`).
    PushTransform { translate: Point, scale: Point },

    /// Pop the most recent transform.
    PopTransform,

    /// Begin a logical group (e.g. a lane). Renderers may use this for
    /// batching, layer separation, or accessibility.
    BeginGroup {
        id: SharedStr,
        label: Option<SharedStr>,
    },

    /// End the current group.
    EndGroup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}
