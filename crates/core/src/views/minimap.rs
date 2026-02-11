use flame_cat_protocol::{Rect, RenderCommand, ThemeToken, Viewport};

use crate::model::Profile;

const MINIMAP_FRAME_HEIGHT: f64 = 3.0;

/// Render a minimap overview of the entire profile. The minimap shows all
/// frames compressed to fit the viewport width, with a viewport indicator
/// overlay showing the currently visible region.
pub fn render_minimap(
    profile: &Profile,
    viewport: &Viewport,
    visible_start_frac: f64,
    visible_end_frac: f64,
) -> Vec<RenderCommand> {
    let duration = profile.duration();
    if duration <= 0.0 {
        return Vec::new();
    }

    let start = profile.metadata.start_time;
    let x_scale = viewport.width / duration;

    let mut commands = Vec::new();
    commands.push(RenderCommand::BeginGroup {
        id: "minimap".to_string(),
        label: Some("Minimap".to_string()),
    });

    // Background
    commands.push(RenderCommand::DrawRect {
        rect: Rect::new(0.0, 0.0, viewport.width, viewport.height),
        color: ThemeToken::MinimapBackground,
        border_color: None,
        label: None,
        frame_id: None,
    });

    // Draw compressed frames
    for frame in &profile.frames {
        let x = (frame.start - start) * x_scale;
        let w = frame.duration() * x_scale;
        let y = f64::from(frame.depth) * MINIMAP_FRAME_HEIGHT;

        if w < 0.3 || y + MINIMAP_FRAME_HEIGHT > viewport.height {
            continue;
        }

        let color = match frame.depth % 4 {
            0 => ThemeToken::FlameHot,
            1 => ThemeToken::FlameWarm,
            2 => ThemeToken::FlameCold,
            _ => ThemeToken::FlameNeutral,
        };

        commands.push(RenderCommand::DrawRect {
            rect: Rect::new(x, y, w, MINIMAP_FRAME_HEIGHT),
            color,
            border_color: None,
            label: None,
            frame_id: None,
        });
    }

    // Viewport indicator overlay
    let vp_x = visible_start_frac * viewport.width;
    let vp_w = (visible_end_frac - visible_start_frac) * viewport.width;
    commands.push(RenderCommand::DrawRect {
        rect: Rect::new(vp_x, 0.0, vp_w, viewport.height),
        color: ThemeToken::MinimapViewport,
        border_color: Some(ThemeToken::Border),
        label: None,
        frame_id: None,
    });

    commands.push(RenderCommand::EndGroup);
    commands
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Frame, ProfileMetadata};

    #[test]
    fn renders_minimap_with_viewport() {
        let profile = Profile {
            metadata: ProfileMetadata {
                name: None,
                start_time: 0.0,
                end_time: 100.0,
                format: "test".to_string(),
            },
            frames: vec![Frame {
                id: 0,
                name: "main".into(),
                start: 0.0,
                end: 100.0,
                depth: 0,
                category: None,
                parent: None,
                self_time: 100.0,
            }],
        };
        let vp = Viewport {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 40.0,
            dpr: 1.0,
        };
        let cmds = render_minimap(&profile, &vp, 0.0, 0.5);
        let rects: Vec<_> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCommand::DrawRect { .. }))
            .collect();
        // Background + frame + viewport indicator
        assert!(rects.len() >= 3);
    }
}
