//! SVG renderer: converts `RenderCommand` lists into standalone SVG strings.

use flame_cat_protocol::{RenderCommand, ThemeToken};

/// Render a list of commands as an SVG document string.
///
/// `width` and `height` define the SVG viewBox dimensions.
/// `dark` selects the color palette.
pub fn render_svg(commands: &[RenderCommand], width: f64, height: f64, dark: bool) -> String {
    let mut svg = String::with_capacity(commands.len() * 200);
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
            // Skip transform/clip/group commands — they don't affect static SVG output
            _ => {}
        }
    }

    svg.push_str("</svg>");
    svg
}

fn resolve_color(token: ThemeToken, dark: bool) -> &'static str {
    if dark {
        match token {
            ThemeToken::FlameHot => "#f44336",
            ThemeToken::FlameWarm => "#ffa726",
            ThemeToken::FlameCold => "#42a5f5",
            ThemeToken::FlameNeutral => "#9575cd",
            ThemeToken::Border | ThemeToken::TableBorder | ThemeToken::AsyncSpanBorder => "#303030",
            ThemeToken::TextPrimary | ThemeToken::ToolbarText | ThemeToken::InlineLabelText => {
                "#ececec"
            }
            ThemeToken::TextSecondary | ThemeToken::TextMuted | ThemeToken::CounterText => {
                "#9e9e9e"
            }
            ThemeToken::LaneBackground | ThemeToken::Background => "#181818",
            ThemeToken::SelectionHighlight | ThemeToken::HoverHighlight => "#448aff",
            ThemeToken::SearchHighlight => "#ffeb3b",
            ThemeToken::BarFill | ThemeToken::CounterFill | ThemeToken::AsyncSpanFill => "#448aff",
            ThemeToken::MarkerLine | ThemeToken::MarkerText => "#ffd600",
            ThemeToken::FrameGood => "#4caf50",
            _ => "#616161",
        }
    } else {
        match token {
            ThemeToken::FlameHot => "#e63946",
            ThemeToken::FlameWarm => "#f4845f",
            ThemeToken::FlameCold => "#457b9d",
            ThemeToken::FlameNeutral => "#adb5bd",
            ThemeToken::Border | ThemeToken::TableBorder | ThemeToken::AsyncSpanBorder => "#dee2e6",
            ThemeToken::TextPrimary | ThemeToken::ToolbarText | ThemeToken::InlineLabelText => {
                "#1a1a2e"
            }
            ThemeToken::TextSecondary | ThemeToken::TextMuted | ThemeToken::CounterText => {
                "#666677"
            }
            ThemeToken::LaneBackground | ThemeToken::Background => "#f8f9fa",
            ThemeToken::SelectionHighlight | ThemeToken::HoverHighlight => "#ffd60a",
            ThemeToken::SearchHighlight => "#00b87a",
            ThemeToken::BarFill | ThemeToken::CounterFill | ThemeToken::AsyncSpanFill => "#adb5bd",
            ThemeToken::MarkerLine | ThemeToken::MarkerText => "#e67e22",
            ThemeToken::FrameGood => "#27ae60",
            _ => "#999999",
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
        assert!(svg.contains("#f44336"));
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
