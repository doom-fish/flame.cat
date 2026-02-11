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
pub fn render_commands(
    painter: &mut egui::Painter,
    commands: &[RenderCommand],
    offset: Pos2,
    mode: ThemeMode,
    search: &str,
) -> RenderResult {
    let mut transform_stack: Vec<Transform> = vec![Transform::identity()];
    let mut clip_stack: Vec<Rect> = Vec::new();
    let mut hit_regions: Vec<HitRegion> = Vec::new();

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

                let fill = theme::resolve(*color, mode);

                // Dim non-matching spans when search is active
                let search_match = search.is_empty()
                    || label.as_ref().is_some_and(|l| {
                        l.as_ref().to_lowercase().contains(&search.to_lowercase())
                    });
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
                        // Truncate: only draw if text fits
                        if galley.size().x <= text_rect.width() + 2.0 {
                            let text_pos = Pos2::new(
                                text_rect.left(),
                                text_rect.center().y - galley.size().y / 2.0,
                            );
                            painter.galley(text_pos, galley, text_color);
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

                painter.text(
                    Pos2::new(x, y),
                    anchor,
                    text.as_ref(),
                    FontId::proportional(size),
                    text_color,
                );
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
    }

    RenderResult { hit_regions }
}
