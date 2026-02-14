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

    let bg = if dark { "#1a1a2e" } else { "#ffffff" };
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
                    let text_color = if dark { "#e0e0e0" } else { "#1a1a2e" };
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

fn resolve_color(token: ThemeToken, dark: bool) -> &'static str {
    if dark {
        // Catppuccin Mocha palette
        match token {
            ThemeToken::FlameHot => "#f38ba8",
            ThemeToken::FlameWarm => "#fab387",
            ThemeToken::FlameCold => "#89b4fa",
            ThemeToken::FlameNeutral => "#cba6f7",
            ThemeToken::Border | ThemeToken::TableBorder | ThemeToken::AsyncSpanBorder => "#313244",
            ThemeToken::LaneBorder => "#45475a",
            ThemeToken::TextPrimary | ThemeToken::ToolbarText | ThemeToken::InlineLabelText => {
                "#cdd6f4"
            }
            ThemeToken::TextSecondary | ThemeToken::TextMuted | ThemeToken::CounterText => {
                "#bac2de"
            }
            ThemeToken::LaneBackground | ThemeToken::Background => "#1e1e2e",
            ThemeToken::Surface | ThemeToken::ToolbarBackground => "#181825",
            ThemeToken::LaneHeaderBackground | ThemeToken::TableHeaderBackground => "#313244",
            ThemeToken::LaneHeaderText => "#a6adc8",
            ThemeToken::SelectionHighlight | ThemeToken::HoverHighlight => "#89b4fa",
            ThemeToken::SearchHighlight => "#f9e2af",
            ThemeToken::BarFill | ThemeToken::CounterFill | ThemeToken::AsyncSpanFill => "#89b4fa",
            ThemeToken::CounterLine => "#74c7ec",
            ThemeToken::MarkerLine | ThemeToken::MarkerText => "#f9e2af",
            ThemeToken::FrameGood => "#a6e3a1",
            ThemeToken::FrameWarning => "#f9e2af",
            ThemeToken::FrameDropped => "#f38ba8",
            ThemeToken::TableRowEven => "#1e1e2e",
            ThemeToken::TableRowOdd => "#181825",
            ThemeToken::ToolbarTabActive => "#45475a",
            ThemeToken::ToolbarTabHover => "#313244",
            ThemeToken::MinimapBackground => "#11111b",
            ThemeToken::MinimapViewport => "#585b70",
            ThemeToken::MinimapDensity => "#89b4fa",
            ThemeToken::MinimapHandle => "#a6adc8",
            ThemeToken::InlineLabelBackground => "#313244",
            ThemeToken::FlowArrow | ThemeToken::FlowArrowHead => "#585b70",
        }
    } else {
        match token {
            ThemeToken::FlameHot => "#e63946",
            ThemeToken::FlameWarm => "#f4845f",
            ThemeToken::FlameCold => "#457b9d",
            ThemeToken::FlameNeutral => "#adb5bd",
            ThemeToken::Border | ThemeToken::TableBorder | ThemeToken::AsyncSpanBorder => "#dee2e6",
            ThemeToken::LaneBorder => "#ced4da",
            ThemeToken::TextPrimary | ThemeToken::ToolbarText | ThemeToken::InlineLabelText => {
                "#1a1a2e"
            }
            ThemeToken::TextSecondary | ThemeToken::TextMuted | ThemeToken::CounterText => {
                "#666677"
            }
            ThemeToken::LaneBackground | ThemeToken::Background => "#f8f9fa",
            ThemeToken::Surface | ThemeToken::ToolbarBackground => "#e9ecef",
            ThemeToken::LaneHeaderBackground | ThemeToken::TableHeaderBackground => "#dee2e6",
            ThemeToken::LaneHeaderText => "#495057",
            ThemeToken::SelectionHighlight | ThemeToken::HoverHighlight => "#ffd60a",
            ThemeToken::SearchHighlight => "#00b87a",
            ThemeToken::BarFill | ThemeToken::CounterFill | ThemeToken::AsyncSpanFill => "#457b9d",
            ThemeToken::CounterLine => "#1d3557",
            ThemeToken::MarkerLine | ThemeToken::MarkerText => "#e67e22",
            ThemeToken::FrameGood => "#27ae60",
            ThemeToken::FrameWarning => "#f39c12",
            ThemeToken::FrameDropped => "#e63946",
            ThemeToken::TableRowEven => "#f8f9fa",
            ThemeToken::TableRowOdd => "#e9ecef",
            ThemeToken::ToolbarTabActive => "#dee2e6",
            ThemeToken::ToolbarTabHover => "#e9ecef",
            ThemeToken::MinimapBackground => "#e9ecef",
            ThemeToken::MinimapViewport => "#adb5bd",
            ThemeToken::MinimapDensity => "#457b9d",
            ThemeToken::MinimapHandle => "#495057",
            ThemeToken::InlineLabelBackground => "#dee2e6",
            ThemeToken::FlowArrow | ThemeToken::FlowArrowHead => "#adb5bd",
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
