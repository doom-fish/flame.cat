use serde::{Deserialize, Serialize};

use crate::theme::ThemeToken;
use crate::types::{Point, Rect};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RenderCommand {
    DrawRect {
        rect: Rect,
        color: ThemeToken,
        label: Option<String>,
        frame_id: Option<u64>,
    },
    DrawText {
        position: Point,
        text: String,
        color: ThemeToken,
        size: f64,
    },
    DrawLine {
        from: Point,
        to: Point,
        color: ThemeToken,
        width: f64,
    },
    SetClip {
        rect: Rect,
    },
    ClearClip,
    PushTransform {
        translate: Point,
        scale: Point,
    },
    PopTransform,
}
