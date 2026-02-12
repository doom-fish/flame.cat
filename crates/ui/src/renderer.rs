use egui::{Align2, CornerRadius, FontId, Pos2, Rect, Stroke, StrokeKind};
use flame_cat_protocol::{RenderCommand, TextAlign, ThemeToken};

use crate::theme::{self, ThemeMode};

/// Transform state for PushTransform/PopTransform.
#[derive(Debug, Clone, Copy)]
struct Transform {
    tx: f64,
    ty: f64,
    sx: f64,
    sy: f64,
}

impl Transform {
    fn identity() -> Self {
        Self {
            tx: 0.0,
            ty: 0.0,
            sx: 1.0,
            sy: 1.0,
        }
    }

    fn apply_x(&self, x: f64) -> f32 {
        (x * self.sx + self.tx) as f32
    }

    fn apply_y(&self, y: f64) -> f32 {
        (y * self.sy + self.ty) as f32
    }

    fn scale_w(&self, w: f64) -> f32 {
        (w * self.sx) as f32
    }

    fn scale_h(&self, h: f64) -> f32 {
        (h * self.sy) as f32
    }
}

/// Holds state needed to find which frame_id the user clicked/hovered on.
pub struct HitRegion {
    pub rect: Rect,
    pub frame_id: u64,
}

/// Result of rendering a command list: includes hit regions for interaction.
pub struct RenderResult {
    pub hit_regions: Vec<HitRegion>,
}

/// Render a list of `RenderCommand` into an egui `Painter`.
///
/// `offset` is the top-left pixel position of the rendering area.
/// `search` is an optional search filter — non-matching spans are dimmed.
/// Returns hit regions for click/hover interaction.
/// How span rectangles are colored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    /// Use theme token from render command (depth-based cycling).
    Theme,
    /// Hash the span label into a consistent hue (color-by-package).
    ByName,
}

pub fn render_commands(
    painter: &mut egui::Painter,
    commands: &[RenderCommand],
    offset: Pos2,
    mode: ThemeMode,
    search: &str,
    color_mode: ColorMode,
) -> RenderResult {
    let mut transform_stack: Vec<Transform> = vec![Transform::identity()];
    let mut clip_stack: Vec<Rect> = Vec::new();
    let mut hit_regions: Vec<HitRegion> = Vec::new();

    let search_lower = search.to_lowercase();

    // Pre-compute which labels match the search to avoid per-rect to_lowercase()
    let search_active = !search_lower.is_empty();
    let matching_labels: std::collections::HashSet<usize> = if search_active {
        commands
            .iter()
            .enumerate()
            .filter_map(|(i, cmd)| {
                if let RenderCommand::DrawRect { label: Some(l), .. } = cmd {
                    if l.as_ref().to_lowercase().contains(&search_lower) {
                        return Some(i);
                    }
                }
                None
            })
            .collect()
    } else {
        std::collections::HashSet::new()
    };

    let mut cmd_index: usize = 0;
    for cmd in commands {
        let tf = transform_stack
            .last()
            .copied()
            .unwrap_or(Transform::identity());
        match cmd {
            RenderCommand::DrawRect {
                rect,
                color,
                border_color,
                label,
                frame_id,
            } => {
                let x = tf.apply_x(rect.x) + offset.x;
                let y = tf.apply_y(rect.y) + offset.y;
                let w = tf.scale_w(rect.w);
                let h = tf.scale_h(rect.h);

                if w < 0.5 || h < 0.5 {
                    continue;
                }

                let egui_rect = Rect::from_min_size(Pos2::new(x, y), egui::vec2(w, h));

                // Cull off-screen
                if !painter.clip_rect().intersects(egui_rect) {
                    continue;
                }

                let fill = match color_mode {
                    ColorMode::ByName => {
                        if let Some(label_text) = label {
                            name_to_color(label_text, mode)
                        } else {
                            theme::resolve(*color, mode)
                        }
                    }
                    ColorMode::Theme => theme::resolve(*color, mode),
                };

                // Dim non-matching spans when search is active
                let search_match = !search_active || matching_labels.contains(&cmd_index);
                let fill = if search_match {
                    fill
                } else {
                    egui::Color32::from_rgba_unmultiplied(fill.r(), fill.g(), fill.b(), 40)
                };

                painter.rect_filled(egui_rect, CornerRadius::ZERO, fill);

                if let Some(bc) = border_color {
                    let stroke_color = theme::resolve(*bc, mode);
                    painter.rect_stroke(
                        egui_rect,
                        CornerRadius::ZERO,
                        Stroke::new(1.0, stroke_color),
                        StrokeKind::Outside,
                    );
                }

                // Draw label text inside the rect
                if let Some(label_text) = label {
                    let label_str: &str = label_text;
                    if !label_str.is_empty() && w > 6.0 && h > 8.0 {
                        let font_size = (h - 4.0).min(11.0).max(6.0);
                        let text_color = theme::resolve(ThemeToken::TextPrimary, mode);
                        let text_rect = egui_rect.shrink2(egui::vec2(2.0, 0.0));
                        let galley = painter.layout_no_wrap(
                            label_str.to_string(),
                            FontId::proportional(font_size),
                            text_color,
                        );
                        let text_pos = Pos2::new(
                            text_rect.left(),
                            text_rect.center().y - galley.size().y / 2.0,
                        );
                        if galley.size().x <= text_rect.width() + 2.0 {
                            painter.galley(text_pos, galley, text_color);
                        } else if text_rect.width() > 20.0 {
                            // Truncate with ellipsis for medium-width spans
                            let avail = text_rect.width() - 8.0;
                            let ratio = avail / galley.size().x;
                            let take = ((label_str.chars().count() as f32 * ratio) as usize).max(1);
                            let truncated: String = label_str.chars().take(take).collect();
                            let ellipsis_galley = painter.layout_no_wrap(
                                format!("{truncated}…"),
                                FontId::proportional(font_size),
                                text_color,
                            );
                            painter.galley(text_pos, ellipsis_galley, text_color);
                        }
                    }
                }

                if let Some(fid) = frame_id {
                    hit_regions.push(HitRegion {
                        rect: egui_rect,
                        frame_id: *fid,
                    });
                }
            }

            RenderCommand::DrawText {
                position,
                text,
                color,
                font_size,
                align,
            } => {
                let x = tf.apply_x(position.x) + offset.x;
                let y = tf.apply_y(position.y) + offset.y;
                let size = *font_size as f32;
                if size < 1.0 {
                    continue;
                }

                let text_color = theme::resolve(*color, mode);
                let anchor = match align {
                    TextAlign::Left => Align2::LEFT_CENTER,
                    TextAlign::Center => Align2::CENTER_CENTER,
                    TextAlign::Right => Align2::RIGHT_CENTER,
                };

                // Background pill behind text for readability over chart fills
                let galley = painter.layout_no_wrap(
                    text.as_ref().to_string(),
                    FontId::proportional(size),
                    text_color,
                );
                let text_pos = match anchor {
                    Align2::LEFT_CENTER => Pos2::new(x, y - galley.size().y / 2.0),
                    Align2::RIGHT_CENTER => {
                        Pos2::new(x - galley.size().x, y - galley.size().y / 2.0)
                    }
                    _ => Pos2::new(x - galley.size().x / 2.0, y - galley.size().y / 2.0),
                };
                let bg_rect = Rect::from_min_size(
                    text_pos - egui::vec2(3.0, 1.0),
                    galley.size() + egui::vec2(6.0, 2.0),
                );
                let bg_color = match mode {
                    ThemeMode::Dark => egui::Color32::from_rgba_unmultiplied(30, 30, 35, 190),
                    ThemeMode::Light => egui::Color32::from_rgba_unmultiplied(250, 250, 252, 210),
                };
                painter.rect_filled(bg_rect, CornerRadius::same(2), bg_color);
                painter.galley(text_pos, galley, text_color);
            }

            RenderCommand::DrawLine {
                from,
                to,
                color,
                width,
            } => {
                let p1 = Pos2::new(tf.apply_x(from.x) + offset.x, tf.apply_y(from.y) + offset.y);
                let p2 = Pos2::new(tf.apply_x(to.x) + offset.x, tf.apply_y(to.y) + offset.y);
                let line_color = theme::resolve(*color, mode);
                painter.line_segment([p1, p2], Stroke::new(*width as f32, line_color));
            }

            RenderCommand::SetClip { rect } => {
                let x = tf.apply_x(rect.x) + offset.x;
                let y = tf.apply_y(rect.y) + offset.y;
                let w = tf.scale_w(rect.w);
                let h = tf.scale_h(rect.h);
                let clip_rect = Rect::from_min_size(Pos2::new(x, y), egui::vec2(w, h));
                clip_stack.push(painter.clip_rect());
                let intersected = painter.clip_rect().intersect(clip_rect);
                painter.set_clip_rect(intersected);
            }

            RenderCommand::ClearClip => {
                if let Some(prev) = clip_stack.pop() {
                    painter.set_clip_rect(prev);
                }
            }

            RenderCommand::PushTransform { translate, scale } => {
                let parent = tf;
                transform_stack.push(Transform {
                    tx: parent.tx + translate.x * parent.sx,
                    ty: parent.ty + translate.y * parent.sy,
                    sx: parent.sx * scale.x,
                    sy: parent.sy * scale.y,
                });
            }

            RenderCommand::PopTransform => {
                if transform_stack.len() > 1 {
                    transform_stack.pop();
                }
            }

            RenderCommand::BeginGroup { .. } | RenderCommand::EndGroup => {
                // Groups are semantic — no visual effect in egui
            }
        }
        cmd_index += 1;
    }

    RenderResult { hit_regions }
}

/// Generate a consistent color from a span name by hashing the "package" prefix.
/// Extracts the first segment before common separators (::, ., /, @) and hashes it.
fn name_to_color(name: &str, mode: ThemeMode) -> egui::Color32 {
    // Extract package/module prefix
    let prefix = name
        .split(|c: char| c == ':' || c == '.' || c == '/' || c == '@' || c == '\\')
        .next()
        .unwrap_or(name);

    // Simple hash → hue
    let mut hash: u32 = 5381;
    for b in prefix.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(u32::from(b));
    }
    let hue = (hash % 360) as f32;

    // HSL → RGB with theme-appropriate saturation/lightness
    let (s, l) = match mode {
        ThemeMode::Dark => (0.55, 0.55),
        ThemeMode::Light => (0.50, 0.62),
    };
    hsl_to_color32(hue, s, l)
}

fn hsl_to_color32(h: f32, s: f32, l: f32) -> egui::Color32 {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = match (h as u32) / 60 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    egui::Color32::from_rgb(
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}
