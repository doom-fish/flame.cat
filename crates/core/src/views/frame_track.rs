use flame_cat_protocol::{
    FrameTiming, Point, Rect, RenderCommand, SharedStr, TextAlign, ThemeToken, Viewport,
};

const FRAME_TRACK_HEIGHT: f64 = 30.0;
const FONT_SIZE: f64 = 9.0;
const FRAME_GAP: f64 = 1.0;

/// 60 FPS target frame budget in microseconds.
const FRAME_BUDGET_60FPS: f64 = 16_667.0;
/// 30 FPS budget in microseconds.
const FRAME_BUDGET_30FPS: f64 = 33_333.0;

/// Render a frame cost track showing per-frame bars colored by cost.
///
/// Green = under 16.67ms (60fps), Yellow = under 33.33ms (30fps), Red = over 33.33ms.
pub fn render_frame_track(
    frames: &[FrameTiming],
    viewport: &Viewport,
    view_start: f64,
    view_end: f64,
) -> Vec<RenderCommand> {
    let duration = view_end - view_start;
    if duration <= 0.0 || frames.is_empty() {
        return Vec::new();
    }

    let height = FRAME_TRACK_HEIGHT.min(viewport.height);
    let x_scale = viewport.width / duration;

    // Find max frame duration for Y scaling
    let max_dur = frames
        .iter()
        .filter(|f| f.start >= view_start && f.start <= view_end)
        .map(|f| f.duration)
        .fold(0.0_f64, f64::max)
        .max(FRAME_BUDGET_60FPS); // ensure budget line is always visible

    let y_scale = (height - FONT_SIZE - 2.0) / max_dur;

    let mut commands = Vec::with_capacity(frames.len() + 6);

    commands.push(RenderCommand::BeginGroup {
        id: "frames".into(),
        label: Some("Frame Cost".into()),
    });

    // Background
    commands.push(RenderCommand::DrawRect {
        rect: Rect::new(0.0, 0.0, viewport.width, height),
        color: ThemeToken::LaneBackground,
        border_color: Some(ThemeToken::LaneBorder),
        label: None,
        frame_id: None,
    });

    // 60fps budget line
    let budget_y = height - FRAME_BUDGET_60FPS * y_scale;
    if budget_y > 0.0 && budget_y < height {
        commands.push(RenderCommand::DrawLine {
            from: Point::new(0.0, budget_y),
            to: Point::new(viewport.width, budget_y),
            color: ThemeToken::FrameWarning,
            width: 0.5,
        });
    }

    // Frame bars
    for frame in frames {
        if frame.end < view_start || frame.start > view_end {
            continue;
        }

        let x = (frame.start - view_start) * x_scale;
        let w = (frame.duration * x_scale - FRAME_GAP).max(1.0);
        let bar_height = frame.duration * y_scale;
        let y = height - bar_height;

        let color = if frame.duration <= FRAME_BUDGET_60FPS {
            ThemeToken::FrameGood
        } else if frame.duration <= FRAME_BUDGET_30FPS {
            ThemeToken::FrameWarning
        } else {
            ThemeToken::FrameDropped
        };

        commands.push(RenderCommand::DrawRect {
            rect: Rect::new(x, y, w, bar_height),
            color,
            border_color: None,
            label: None,
            frame_id: None,
        });

        // Duration label on wide frames
        if w > 40.0 {
            let label = if frame.duration >= 1000.0 {
                format!("{:.1}ms", frame.duration / 1000.0)
            } else {
                format!("{:.0}µs", frame.duration)
            };
            commands.push(RenderCommand::DrawText {
                position: Point::new(x + w / 2.0, y - 1.0),
                text: SharedStr::from(label.as_str()),
                color: ThemeToken::TextMuted,
                font_size: FONT_SIZE,
                align: TextAlign::Center,
            });
        }
    }

    // Title
    commands.push(RenderCommand::DrawText {
        position: Point::new(2.0, FONT_SIZE + 1.0),
        text: "Frames".into(),
        color: ThemeToken::TextSecondary,
        font_size: FONT_SIZE,
        align: TextAlign::Left,
    });

    commands.push(RenderCommand::EndGroup);
    commands
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_frame_bars() {
        let frames = vec![
            FrameTiming {
                start: 0.0,
                end: 16_000.0,
                duration: 16_000.0,
                dropped: false,
            },
            FrameTiming {
                start: 16_000.0,
                end: 50_000.0,
                duration: 34_000.0,
                dropped: true,
            },
            FrameTiming {
                start: 50_000.0,
                end: 66_000.0,
                duration: 16_000.0,
                dropped: false,
            },
        ];
        let vp = Viewport {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 30.0,
            dpr: 1.0,
        };
        let cmds = render_frame_track(&frames, &vp, 0.0, 70_000.0);
        assert!(!cmds.is_empty());
        let rects: Vec<_> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCommand::DrawRect { .. }))
            .collect();
        assert!(rects.len() >= 4); // bg + 3 frame bars
    }

    #[test]
    fn empty_frames_returns_empty() {
        let vp = Viewport {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 30.0,
            dpr: 1.0,
        };
        let cmds = render_frame_track(&[], &vp, 0.0, 100.0);
        assert!(cmds.is_empty());
    }
}
