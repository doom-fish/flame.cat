use flame_cat_protocol::{Point, Rect, RenderCommand, SharedStr, TextAlign, ThemeToken, Viewport};

const AXIS_HEIGHT: f64 = 24.0;
const MAJOR_TICK_HEIGHT: f64 = 10.0;
const MEDIUM_TICK_HEIGHT: f64 = 6.0;
const MINOR_TICK_HEIGHT: f64 = 3.0;
const FONT_SIZE: f64 = 10.0;
const LABEL_Y: f64 = 20.0;
const MIN_MAJOR_SPACING_PX: f64 = 80.0;

/// Render a time axis ruler with major/medium/minor ticks and labels.
///
/// `view_start` and `view_end` are in microseconds (absolute timestamps).
/// Returns render commands for the axis bar + ticks + labels, plus
/// vertical gridlines extending `grid_height` below the axis.
pub fn render_time_axis(
    viewport: &Viewport,
    view_start: f64,
    view_end: f64,
    grid_height: f64,
) -> Vec<RenderCommand> {
    let duration = view_end - view_start;
    if duration <= 0.0 {
        return Vec::new();
    }

    let width = viewport.width;
    let x_scale = width / duration;
    let mut commands = Vec::with_capacity(64);

    // Background bar
    commands.push(RenderCommand::DrawRect {
        rect: Rect::new(0.0, 0.0, width, AXIS_HEIGHT),
        color: ThemeToken::LaneHeaderBackground,
        border_color: Some(ThemeToken::LaneBorder),
        label: None,
        frame_id: None,
    });

    // Calculate tick spacing: find a "nice" interval in microseconds
    let (major_interval, subdivisions) = nice_interval(duration, width);

    let medium_interval = major_interval / subdivisions as f64;
    let minor_interval = medium_interval / 2.0;

    // Align to interval boundaries
    let first_major = (view_start / major_interval).floor() * major_interval;

    // Draw minor ticks
    let first_minor = (view_start / minor_interval).floor() * minor_interval;
    let mut t = first_minor;
    while t <= view_end {
        let x = (t - view_start) * x_scale;
        if x >= 0.0 && x <= width {
            let is_major = is_aligned(t, major_interval, first_major);
            let is_medium = !is_major && is_aligned(t, medium_interval, first_major);

            if !is_major && !is_medium {
                commands.push(RenderCommand::DrawLine {
                    from: Point::new(x, AXIS_HEIGHT - MINOR_TICK_HEIGHT),
                    to: Point::new(x, AXIS_HEIGHT),
                    color: ThemeToken::TextMuted,
                    width: 0.5,
                });
            }
        }
        t += minor_interval;
    }

    // Draw medium ticks
    let first_medium = (view_start / medium_interval).floor() * medium_interval;
    t = first_medium;
    while t <= view_end {
        let x = (t - view_start) * x_scale;
        if x >= 0.0 && x <= width {
            let is_major = is_aligned(t, major_interval, first_major);
            if !is_major {
                commands.push(RenderCommand::DrawLine {
                    from: Point::new(x, AXIS_HEIGHT - MEDIUM_TICK_HEIGHT),
                    to: Point::new(x, AXIS_HEIGHT),
                    color: ThemeToken::TextMuted,
                    width: 0.5,
                });
            }
        }
        t += medium_interval;
    }

    // Draw major ticks with labels + gridlines
    t = first_major;
    while t <= view_end {
        let x = (t - view_start) * x_scale;
        if x >= 0.0 && x <= width {
            // Major tick mark
            commands.push(RenderCommand::DrawLine {
                from: Point::new(x, AXIS_HEIGHT - MAJOR_TICK_HEIGHT),
                to: Point::new(x, AXIS_HEIGHT),
                color: ThemeToken::LaneBorder,
                width: 1.0,
            });

            // Time label
            let label = format_time_label(t, major_interval);
            commands.push(RenderCommand::DrawText {
                position: Point::new(x + 3.0, LABEL_Y - 8.0),
                text: SharedStr::from(label.as_str()),
                color: ThemeToken::TextPrimary,
                font_size: FONT_SIZE,
                align: TextAlign::Left,
            });

            // Vertical gridline through all lanes
            if grid_height > 0.0 {
                commands.push(RenderCommand::DrawLine {
                    from: Point::new(x, AXIS_HEIGHT),
                    to: Point::new(x, AXIS_HEIGHT + grid_height),
                    color: ThemeToken::LaneBorder,
                    width: 0.5,
                });
            }
        }
        t += major_interval;
    }

    commands
}

/// Check if time `t` is approximately aligned with `interval` starting from `base`.
fn is_aligned(t: f64, interval: f64, base: f64) -> bool {
    let offset = (t - base) / interval;
    (offset - offset.round()).abs() < 0.001
}

/// Choose a "nice" major tick interval in microseconds given the visible duration
/// and pixel width. Returns (major_interval_us, subdivisions).
fn nice_interval(duration_us: f64, width_px: f64) -> (f64, u32) {
    // Target: roughly one major tick per MIN_MAJOR_SPACING_PX pixels
    let target_count = (width_px / MIN_MAJOR_SPACING_PX).max(2.0);
    let raw_interval = duration_us / target_count;

    // Nice intervals in microseconds: 1µs, 2µs, 5µs, 10µs, ... 1s, 2s, 5s, 10s ...
    let nice_values: &[(f64, u32)] = &[
        (0.1, 2),
        (0.2, 2),
        (0.5, 5),
        (1.0, 2),
        (2.0, 2),
        (5.0, 5),
        (10.0, 2),
        (20.0, 2),
        (50.0, 5),
        (100.0, 2),
        (200.0, 2),
        (500.0, 5),
        (1_000.0, 2),       // 1ms
        (2_000.0, 2),
        (5_000.0, 5),
        (10_000.0, 2),      // 10ms
        (20_000.0, 2),
        (50_000.0, 5),
        (100_000.0, 2),     // 100ms
        (200_000.0, 2),
        (500_000.0, 5),
        (1_000_000.0, 2),   // 1s
        (2_000_000.0, 2),
        (5_000_000.0, 5),
        (10_000_000.0, 2),  // 10s
        (20_000_000.0, 2),
        (30_000_000.0, 3),  // 30s
        (60_000_000.0, 2),  // 1min
    ];

    for &(interval, subs) in nice_values {
        if interval >= raw_interval {
            return (interval, subs);
        }
    }

    // Fallback for very long traces
    let magnitude = 10.0_f64.powf(raw_interval.log10().floor());
    (magnitude, 2)
}

/// Format a timestamp in microseconds as a human-readable label.
fn format_time_label(us: f64, _interval: f64) -> String {
    let abs = us.abs();
    if abs >= 60_000_000.0 {
        let mins = (us / 60_000_000.0).floor();
        let secs = (us - mins * 60_000_000.0) / 1_000_000.0;
        format!("{:.0}m{:.1}s", mins, secs)
    } else if abs >= 1_000_000.0 {
        format!("{:.3}s", us / 1_000_000.0)
    } else if abs >= 1_000.0 {
        format!("{:.2}ms", us / 1_000.0)
    } else if abs >= 1.0 {
        format!("{:.1}µs", us)
    } else {
        format!("{:.0}ns", us * 1000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nice_interval_selects_reasonable_value() {
        // 1 second visible in 800px → ~10 major ticks → 100ms intervals
        let (interval, _subs) = nice_interval(1_000_000.0, 800.0);
        assert!(interval >= 50_000.0 && interval <= 200_000.0, "interval={interval}");
    }

    #[test]
    fn renders_ticks_and_labels() {
        let vp = Viewport {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 24.0,
            dpr: 1.0,
        };
        let cmds = render_time_axis(&vp, 0.0, 1_000_000.0, 400.0);
        assert!(!cmds.is_empty());

        // Should have background rect
        let rects: Vec<_> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCommand::DrawRect { .. }))
            .collect();
        assert!(!rects.is_empty());

        // Should have text labels
        let texts: Vec<_> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCommand::DrawText { .. }))
            .collect();
        assert!(!texts.is_empty());

        // Should have gridlines
        let lines: Vec<_> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCommand::DrawLine { .. }))
            .collect();
        assert!(lines.len() >= 3);
    }

    #[test]
    fn format_labels() {
        assert_eq!(format_time_label(500.0, 100.0), "500.0µs");
        assert_eq!(format_time_label(1_500.0, 1000.0), "1.50ms");
        assert_eq!(format_time_label(1_500_000.0, 1_000_000.0), "1.500s");
    }
}
