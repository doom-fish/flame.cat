use flame_cat_protocol::{
    CounterTrack, Point, Rect, RenderCommand, SharedStr, TextAlign, ThemeToken, Viewport,
};

const COUNTER_TRACK_HEIGHT: f64 = 60.0;
const LABEL_PADDING: f64 = 4.0;
const FONT_SIZE: f64 = 10.0;

/// Render a single counter track as an area chart.
///
/// Returns render commands that draw a filled area chart of the counter's
/// time-series samples within the given time window.
pub fn render_counter_track(
    counter: &CounterTrack,
    viewport: &Viewport,
    view_start: f64,
    view_end: f64,
) -> Vec<RenderCommand> {
    let duration = view_end - view_start;
    if duration <= 0.0 || counter.samples.is_empty() {
        return Vec::new();
    }

    let height = COUNTER_TRACK_HEIGHT.min(viewport.height);
    let x_scale = viewport.width / duration;

    // Find min/max in the visible range for Y scaling
    let (min_val, max_val) = {
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        for s in &counter.samples {
            if s.ts >= view_start && s.ts <= view_end {
                lo = lo.min(s.value);
                hi = hi.max(s.value);
            }
        }
        // Include one sample before and after visible range for continuity
        if let Some(before) = counter.samples.iter().rev().find(|s| s.ts < view_start) {
            lo = lo.min(before.value);
            hi = hi.max(before.value);
        }
        if let Some(after) = counter.samples.iter().find(|s| s.ts > view_end) {
            lo = lo.min(after.value);
            hi = hi.max(after.value);
        }
        if lo == hi {
            (lo - 1.0, hi + 1.0) // avoid zero range
        } else {
            (lo.min(0.0), hi) // anchor at 0 when all positive
        }
    };

    let y_range = max_val - min_val;
    let y_scale = (height - FONT_SIZE - LABEL_PADDING) / y_range;

    let mut commands = Vec::with_capacity(counter.samples.len() + 6);

    commands.push(RenderCommand::BeginGroup {
        id: SharedStr::from(format!("counter-{}", counter.name).as_str()),
        label: Some(counter.name.clone()),
    });

    // Background
    commands.push(RenderCommand::DrawRect {
        rect: Rect::new(0.0, 0.0, viewport.width, height),
        color: ThemeToken::LaneBackground,
        border_color: Some(ThemeToken::LaneBorder),
        label: None,
        frame_id: None,
    });

    // Draw area chart as a series of filled rectangles (step chart)
    // Each sample holds until the next sample (step function)
    let visible: Vec<_> = counter
        .samples
        .iter()
        .filter(|s| s.ts >= view_start - duration * 0.1 && s.ts <= view_end + duration * 0.1)
        .collect();

    for i in 0..visible.len() {
        let sample = visible[i];
        let next_ts = if i + 1 < visible.len() {
            visible[i + 1].ts
        } else {
            view_end
        };

        let x = (sample.ts - view_start) * x_scale;
        let w = (next_ts - sample.ts) * x_scale;
        let bar_height = (sample.value - min_val) * y_scale;
        let y = height - bar_height;

        if w < 0.1 {
            continue;
        }

        // Area fill
        commands.push(RenderCommand::DrawRect {
            rect: Rect::new(x, y, w, bar_height),
            color: ThemeToken::CounterFill,
            border_color: None,
            label: None,
            frame_id: None,
        });

        // Top edge line
        commands.push(RenderCommand::DrawLine {
            from: Point::new(x, y),
            to: Point::new(x + w, y),
            color: ThemeToken::CounterLine,
            width: 1.0,
        });
    }

    // Title label
    commands.push(RenderCommand::DrawText {
        position: Point::new(LABEL_PADDING, FONT_SIZE + LABEL_PADDING),
        text: counter.name.clone(),
        color: ThemeToken::CounterText,
        font_size: FONT_SIZE,
        align: TextAlign::Left,
    });

    // Max value label
    let max_label = format_counter_value(max_val, &counter.unit);
    commands.push(RenderCommand::DrawText {
        position: Point::new(viewport.width - LABEL_PADDING, FONT_SIZE + LABEL_PADDING),
        text: SharedStr::from(max_label.as_str()),
        color: ThemeToken::TextMuted,
        font_size: FONT_SIZE,
        align: TextAlign::Right,
    });

    commands.push(RenderCommand::EndGroup);
    commands
}

/// Format a counter value with appropriate units.
fn format_counter_value(value: f64, unit: &flame_cat_protocol::CounterUnit) -> String {
    use flame_cat_protocol::CounterUnit;
    match unit {
        CounterUnit::Bytes => {
            if value >= 1_073_741_824.0 {
                format!("{:.1} GB", value / 1_073_741_824.0)
            } else if value >= 1_048_576.0 {
                format!("{:.1} MB", value / 1_048_576.0)
            } else if value >= 1024.0 {
                format!("{:.1} KB", value / 1024.0)
            } else {
                format!("{:.0} B", value)
            }
        }
        CounterUnit::Percent => format!("{:.1}%", value),
        CounterUnit::Microseconds => {
            if value >= 1_000_000.0 {
                format!("{:.2}s", value / 1_000_000.0)
            } else if value >= 1000.0 {
                format!("{:.1}ms", value / 1000.0)
            } else {
                format!("{:.0}µs", value)
            }
        }
        CounterUnit::Milliseconds => {
            if value >= 1000.0 {
                format!("{:.2}s", value / 1000.0)
            } else {
                format!("{:.1}ms", value)
            }
        }
        CounterUnit::Count | CounterUnit::None => {
            if value >= 1_000_000.0 {
                format!("{:.1}M", value / 1_000_000.0)
            } else if value >= 1000.0 {
                format!("{:.1}K", value / 1000.0)
            } else {
                format!("{:.0}", value)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flame_cat_protocol::{CounterSample, CounterUnit};

    #[test]
    fn renders_counter_area_chart() {
        let counter = CounterTrack {
            name: "JS Heap Size".into(),
            unit: CounterUnit::Bytes,
            samples: vec![
                CounterSample {
                    ts: 0.0,
                    value: 1_048_576.0,
                },
                CounterSample {
                    ts: 50.0,
                    value: 2_097_152.0,
                },
                CounterSample {
                    ts: 100.0,
                    value: 1_572_864.0,
                },
            ],
        };
        let vp = Viewport {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 60.0,
            dpr: 1.0,
        };
        let cmds = render_counter_track(&counter, &vp, 0.0, 100.0);

        // Should have: BeginGroup, Background, 3 area rects, 3 lines, 2 texts, EndGroup
        assert!(!cmds.is_empty());
        let rects: Vec<_> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCommand::DrawRect { .. }))
            .collect();
        assert!(rects.len() >= 3); // bg + 2 visible data rects (last sample at view_end has zero width)
    }

    #[test]
    fn empty_counter_returns_empty() {
        let counter = CounterTrack {
            name: "empty".into(),
            unit: CounterUnit::Count,
            samples: vec![],
        };
        let vp = Viewport {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 60.0,
            dpr: 1.0,
        };
        let cmds = render_counter_track(&counter, &vp, 0.0, 100.0);
        assert!(cmds.is_empty());
    }

    #[test]
    fn format_bytes() {
        assert_eq!(format_counter_value(500.0, &CounterUnit::Bytes), "500 B");
        assert_eq!(
            format_counter_value(1_048_576.0, &CounterUnit::Bytes),
            "1.0 MB"
        );
        assert_eq!(
            format_counter_value(1_073_741_824.0, &CounterUnit::Bytes),
            "1.0 GB"
        );
    }
}
