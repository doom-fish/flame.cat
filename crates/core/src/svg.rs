//! SVG renderer: converts `RenderCommand` lists into standalone SVG strings.

use flame_cat_protocol::{RenderCommand, ThemeToken};

/// Render a list of commands as an SVG document string.
///
/// `width` and `height` define the SVG viewBox dimensions.
/// `dark` selects the color palette.
pub fn render_svg(commands: &[RenderCommand], width: f64, height: f64, dark: bool) -> String {
    let mut svg = String::with_capacity(commands.len() * 200);
    let mut clip_counter = 0_u32;
    let mut clip_depth = 0_u32;
    let mut group_depth = 0_u32;
    svg.push_str(&format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}" width="{width}" height="{height}" style="font-family:system-ui,-apple-system,sans-serif;font-size:11px">"#,
    ));

    let bg = resolve_color(ThemeToken::Background, dark);
    svg.push_str(&format!(
        r#"<rect width="{width}" height="{height}" fill="{bg}"/>"#,
    ));

    for cmd in commands {
        match cmd {
            RenderCommand::DrawRect {
                rect, color, label, ..
            } => {
                let fill = resolve_color(*color, dark);
                svg.push_str(&format!(
                    r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{fill}" rx="1">"#,
                    rect.x, rect.y, rect.w, rect.h,
                ));
                if let Some(label) = label {
                    svg.push_str(&format!("<title>{}</title>", escape_xml(label)));
                }
                svg.push_str("</rect>");

                // Render text label if rect is wide enough
                if let Some(label) = label
                    && rect.w > 30.0
                {
                    let text_color = resolve_color(ThemeToken::TextPrimary, dark);
                    let tx = rect.x + 3.0;
                    let ty = rect.y + rect.h * 0.75;
                    let max_chars = (rect.w / 7.0) as usize;
                    let text = if label.chars().count() > max_chars && max_chars > 2 {
                        let truncated: String = label.chars().take(max_chars - 1).collect();
                        format!("{truncated}…")
                    } else {
                        label.to_string()
                    };
                    svg.push_str(&format!(
                        r#"<text x="{tx}" y="{ty}" fill="{text_color}" style="pointer-events:none">{}</text>"#,
                        escape_xml(&text),
                    ));
                }
            }
            RenderCommand::DrawLine {
                from,
                to,
                color,
                width: line_width,
            } => {
                let stroke = resolve_color(*color, dark);
                svg.push_str(&format!(
                    r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{stroke}" stroke-width="{line_width}"/>"#,
                    from.x, from.y, to.x, to.y,
                ));
            }
            RenderCommand::DrawText {
                text,
                position,
                color,
                ..
            } => {
                let fill = resolve_color(*color, dark);
                svg.push_str(&format!(
                    r#"<text x="{}" y="{}" fill="{fill}">{}</text>"#,
                    position.x,
                    position.y,
                    escape_xml(text),
                ));
            }
            RenderCommand::SetClip { rect } => {
                clip_counter += 1;
                let clip_id = format!("clip{clip_counter}");
                svg.push_str(&format!(
                    r#"<clipPath id="{clip_id}"><rect x="{}" y="{}" width="{}" height="{}"/></clipPath><g clip-path="url(#{clip_id})">"#,
                    rect.x, rect.y, rect.w, rect.h,
                ));
                clip_depth += 1;
            }
            RenderCommand::ClearClip => {
                if clip_depth > 0 {
                    svg.push_str("</g>");
                    clip_depth -= 1;
                }
            }
            RenderCommand::BeginGroup { .. } => {
                svg.push_str("<g>");
                group_depth += 1;
            }
            RenderCommand::EndGroup => {
                if group_depth > 0 {
                    svg.push_str("</g>");
                    group_depth -= 1;
                }
            }
            _ => {}
        }
    }

    // Close any unclosed clips and groups
    for _ in 0..clip_depth {
        svg.push_str("</g>");
    }
    for _ in 0..group_depth {
        svg.push_str("</g>");
    }

    svg.push_str("</svg>");
    svg
}

/// Map ThemeToken to hex color string, matching crates/ui/src/theme.rs exactly.
fn resolve_color(token: ThemeToken, dark: bool) -> &'static str {
    if dark {
        // Catppuccin Mocha palette — must match theme.rs resolve_dark()
        match token {
            ThemeToken::FlameHot => "#f38ba8",
            ThemeToken::FlameWarm => "#fab387",
            ThemeToken::FlameCold => "#89b4fa",
            ThemeToken::FlameNeutral => "#cba6f7",
            ThemeToken::LaneBackground => "#1e1e2e",
            ThemeToken::LaneBorder => "#313244",
            ThemeToken::LaneHeaderBackground => "#181825",
            ThemeToken::LaneHeaderText => "#cdd6f4",
            ThemeToken::TextPrimary => "#cdd6f4",
            ThemeToken::TextSecondary => "#bac2de",
            ThemeToken::TextMuted => "#a6adc8",
            ThemeToken::SelectionHighlight => "#89b4fa",
            ThemeToken::HoverHighlight => "#cdd6f4",
            ThemeToken::Background => "#11111b",
            ThemeToken::Surface => "#181825",
            ThemeToken::Border => "#313244",
            ThemeToken::ToolbarBackground => "#181825",
            ThemeToken::ToolbarText => "#cdd6f4",
            ThemeToken::ToolbarTabActive => "#89b4fa",
            ThemeToken::ToolbarTabHover => "#cdd6f4",
            ThemeToken::MinimapBackground => "#11111b",
            ThemeToken::MinimapViewport => "#89b4fa",
            ThemeToken::MinimapDensity => "#b4befe",
            ThemeToken::MinimapHandle => "#b4befe",
            ThemeToken::InlineLabelText => "#cdd6f4",
            ThemeToken::InlineLabelBackground => "#1e1e2e",
            ThemeToken::TableRowEven => "#1e1e2e",
            ThemeToken::TableRowOdd => "#181825",
            ThemeToken::TableHeaderBackground => "#313244",
            ThemeToken::TableBorder => "#45475a",
            ThemeToken::BarFill => "#89b4fa",
            ThemeToken::SearchHighlight => "#f9e2af",
            ThemeToken::CounterFill => "#74c7ec",
            ThemeToken::CounterLine => "#74c7ec",
            ThemeToken::CounterText => "#bac2de",
            ThemeToken::MarkerLine => "#f9e2af",
            ThemeToken::MarkerText => "#f9e2af",
            ThemeToken::AsyncSpanFill => "#94e2d5",
            ThemeToken::AsyncSpanBorder => "#74c7ec",
            ThemeToken::FrameGood => "#a6e3a1",
            ThemeToken::FrameWarning => "#f9e2af",
            ThemeToken::FrameDropped => "#f38ba8",
            ThemeToken::FlowArrow | ThemeToken::FlowArrowHead => "#6c7086",
        }
    } else {
        // Light palette — must match theme.rs resolve_light()
        match token {
            ThemeToken::FlameHot => "#dc3c14",
            ThemeToken::FlameWarm => "#e69614",
            ThemeToken::FlameCold => "#2878c8",
            ThemeToken::FlameNeutral => "#788caa",
            ThemeToken::LaneBackground => "#fafafc",
            ThemeToken::LaneBorder => "#d2d2dc",
            ThemeToken::LaneHeaderBackground => "#f0f0f5",
            ThemeToken::LaneHeaderText => "#282832",
            ThemeToken::TextPrimary => "#14141e",
            ThemeToken::TextSecondary => "#505064",
            ThemeToken::TextMuted => "#64646e",
            ThemeToken::SelectionHighlight => "#4287f5",
            ThemeToken::HoverHighlight => "#000000",
            ThemeToken::Background => "#ffffff",
            ThemeToken::Surface => "#f5f5f8",
            ThemeToken::Border => "#d2d2dc",
            ThemeToken::ToolbarBackground => "#f8f8fa",
            ThemeToken::ToolbarText => "#282832",
            ThemeToken::ToolbarTabActive => "#326edc",
            ThemeToken::ToolbarTabHover => "#000000",
            ThemeToken::MinimapBackground => "#f0f0f5",
            ThemeToken::MinimapViewport => "#326edc",
            ThemeToken::MinimapDensity => "#326edc",
            ThemeToken::MinimapHandle => "#2850b4",
            ThemeToken::InlineLabelText => "#282832",
            ThemeToken::InlineLabelBackground => "#f0f0f5",
            ThemeToken::TableRowEven => "#ffffff",
            ThemeToken::TableRowOdd => "#f5f5f8",
            ThemeToken::TableHeaderBackground => "#ebebf0",
            ThemeToken::TableBorder => "#d2d2dc",
            ThemeToken::BarFill => "#326edc",
            ThemeToken::SearchHighlight => "#ffc832",
            ThemeToken::CounterFill => "#326edc",
            ThemeToken::CounterLine => "#326edc",
            ThemeToken::CounterText => "#505064",
            ThemeToken::MarkerLine => "#c89614",
            ThemeToken::MarkerText => "#96640a",
            ThemeToken::AsyncSpanFill => "#508cc8",
            ThemeToken::AsyncSpanBorder => "#326eb4",
            ThemeToken::FrameGood => "#388e3c",
            ThemeToken::FrameWarning => "#e6aa00",
            ThemeToken::FrameDropped => "#d32f2f",
            ThemeToken::FlowArrow | ThemeToken::FlowArrowHead => "#3278dc",
        }
    }
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use flame_cat_protocol::Rect;

    #[test]
    fn basic_svg_output() {
        let commands = vec![RenderCommand::DrawRect {
            rect: Rect::new(10.0, 20.0, 100.0, 18.0),
            color: ThemeToken::FlameHot,
            border_color: None,
            label: Some("main".into()),
            frame_id: Some(1),
        }];
        let svg = render_svg(&commands, 800.0, 400.0, true);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains("main"));
        assert!(svg.contains("#f38ba8"));
    }

    #[test]
    fn escapes_xml_entities() {
        let commands = vec![RenderCommand::DrawRect {
            rect: Rect::new(0.0, 0.0, 200.0, 18.0),
            color: ThemeToken::FlameHot,
            border_color: None,
            label: Some("fn<T>(&self)".into()),
            frame_id: None,
        }];
        let svg = render_svg(&commands, 400.0, 100.0, false);
        assert!(svg.contains("fn&lt;T&gt;(&amp;self)"));
    }
}
