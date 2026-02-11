use std::collections::HashMap;

use flame_cat_protocol::{
    Point, Rect, RenderCommand, SharedStr, TextAlign, ThemeToken, Viewport, VisualProfile,
};

const ROW_HEIGHT: f64 = 24.0;
const HEADER_ROW_HEIGHT: f64 = 28.0;

/// A single row in the ranked table.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RankedEntry {
    pub name: SharedStr,
    pub self_time: f64,
    pub total_time: f64,
    pub count: u32,
}

/// Sort field for the ranked view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RankedSort {
    SelfTime,
    TotalTime,
    Name,
    Count,
}

/// Aggregate all spans by name and produce render commands for a table layout.
pub fn render_ranked(
    profile: &VisualProfile,
    viewport: &Viewport,
    sort: RankedSort,
    ascending: bool,
) -> Vec<RenderCommand> {
    let entries = aggregate_spans(profile, sort, ascending);
    let total_duration = profile.duration().max(1.0);

    let mut commands = Vec::with_capacity(entries.len() * 6 + 4);
    commands.push(RenderCommand::BeginGroup {
        id: "ranked".into(),
        label: Some("Ranked".into()),
    });

    // Column layout: Symbol Name | Self | Total | Count
    let col_self_x = viewport.width * 0.5;
    let col_total_x = viewport.width * 0.68;
    let col_count_x = viewport.width * 0.86;

    // Header row
    commands.push(RenderCommand::DrawRect {
        rect: Rect::new(0.0, 0.0, viewport.width, HEADER_ROW_HEIGHT),
        color: ThemeToken::TableHeaderBackground,
        border_color: Some(ThemeToken::TableBorder),
        label: None,
        frame_id: None,
    });

    let header_y = HEADER_ROW_HEIGHT / 2.0 + 4.0;
    for (text, x) in [
        ("Symbol Name", 8.0),
        ("Self", col_self_x + 4.0),
        ("Total", col_total_x + 4.0),
        ("Count", col_count_x + 4.0),
    ] {
        commands.push(RenderCommand::DrawText {
            position: Point { x, y: header_y },
            text: text.into(),
            color: ThemeToken::TextPrimary,
            font_size: 12.0,
            align: TextAlign::Left,
        });
    }

    // Data rows
    let y_start = HEADER_ROW_HEIGHT;
    for (i, entry) in entries.iter().enumerate() {
        let y = y_start + (i as f64) * ROW_HEIGHT;

        // Skip rows outside viewport
        if y + ROW_HEIGHT < viewport.y || y > viewport.y + viewport.height {
            continue;
        }

        let row_color = if i % 2 == 0 {
            ThemeToken::TableRowEven
        } else {
            ThemeToken::TableRowOdd
        };

        // Row background
        commands.push(RenderCommand::DrawRect {
            rect: Rect::new(0.0, y, viewport.width, ROW_HEIGHT),
            color: row_color,
            border_color: None,
            label: None,
            frame_id: None,
        });

        let text_y = y + ROW_HEIGHT / 2.0 + 4.0;

        // Symbol name
        commands.push(RenderCommand::DrawText {
            position: Point { x: 8.0, y: text_y },
            text: entry.name.clone(),
            color: ThemeToken::TextPrimary,
            font_size: 11.0,
            align: TextAlign::Left,
        });

        // Self time + bar
        let self_pct = entry.self_time / total_duration;
        let bar_max_w = viewport.width * 0.16;
        commands.push(RenderCommand::DrawRect {
            rect: Rect::new(
                col_self_x + 2.0,
                y + ROW_HEIGHT - 4.0,
                bar_max_w * self_pct,
                2.0,
            ),
            color: ThemeToken::BarFill,
            border_color: None,
            label: None,
            frame_id: None,
        });
        commands.push(RenderCommand::DrawText {
            position: Point {
                x: col_self_x + 4.0,
                y: text_y,
            },
            text: format_time(entry.self_time).into(),
            color: ThemeToken::TextSecondary,
            font_size: 11.0,
            align: TextAlign::Left,
        });

        // Total time + bar
        let total_pct = entry.total_time / total_duration;
        commands.push(RenderCommand::DrawRect {
            rect: Rect::new(
                col_total_x + 2.0,
                y + ROW_HEIGHT - 4.0,
                bar_max_w * total_pct,
                2.0,
            ),
            color: ThemeToken::BarFill,
            border_color: None,
            label: None,
            frame_id: None,
        });
        commands.push(RenderCommand::DrawText {
            position: Point {
                x: col_total_x + 4.0,
                y: text_y,
            },
            text: format_time(entry.total_time).into(),
            color: ThemeToken::TextSecondary,
            font_size: 11.0,
            align: TextAlign::Left,
        });

        // Count
        commands.push(RenderCommand::DrawText {
            position: Point {
                x: col_count_x + 4.0,
                y: text_y,
            },
            text: SharedStr::from(entry.count.to_string()),
            color: ThemeToken::TextMuted,
            font_size: 11.0,
            align: TextAlign::Left,
        });
    }

    commands.push(RenderCommand::EndGroup);
    commands
}

/// Compute ranked entries from WASM for the table/detail views.
pub fn get_ranked_entries(
    profile: &VisualProfile,
    sort: RankedSort,
    ascending: bool,
) -> Vec<RankedEntry> {
    aggregate_spans(profile, sort, ascending)
}

fn aggregate_spans(profile: &VisualProfile, sort: RankedSort, ascending: bool) -> Vec<RankedEntry> {
    let mut by_name: HashMap<&str, (SharedStr, f64, f64, u32)> = HashMap::new();

    for span in profile.all_spans() {
        let entry = by_name
            .entry(&span.name)
            .or_insert_with(|| (span.name.clone(), 0.0, 0.0, 0));
        entry.1 += span.self_value;
        entry.2 += span.duration();
        entry.3 += 1;
    }

    let mut entries: Vec<RankedEntry> = Vec::with_capacity(by_name.len());
    entries.extend(
        by_name
            .into_values()
            .map(|(name, self_time, total_time, count)| RankedEntry {
                name,
                self_time,
                total_time,
                count,
            }),
    );

    match sort {
        RankedSort::SelfTime => entries.sort_by(|a, b| b.self_time.total_cmp(&a.self_time)),
        RankedSort::TotalTime => entries.sort_by(|a, b| b.total_time.total_cmp(&a.total_time)),
        RankedSort::Name => entries.sort_by(|a, b| a.name.cmp(&b.name)),
        RankedSort::Count => entries.sort_by(|a, b| b.count.cmp(&a.count)),
    }

    if ascending {
        entries.reverse();
    }

    entries
}

fn format_time(us: f64) -> String {
    if us >= 1_000_000.0 {
        format!("{:.2}s", us / 1_000_000.0)
    } else if us >= 1_000.0 {
        format!("{:.1}ms", us / 1_000.0)
    } else {
        format!("{:.0}µs", us)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flame_cat_protocol::{
        ProfileMeta, SharedStr, SourceFormat, Span, SpanKind, ThreadGroup, ValueUnit, Viewport,
    };

    #[test]
    fn aggregates_by_name() {
        let profile = VisualProfile {
            meta: ProfileMeta {
                name: None,
                source_format: SourceFormat::Unknown,
                value_unit: ValueUnit::Microseconds,
                total_value: 100.0,
                start_time: 0.0,
                end_time: 100.0,
                time_domain: None,
            },
            threads: vec![ThreadGroup {
                id: 0,
                name: "Main".into(),
                sort_key: 0,
                spans: vec![
                    Span {
                        id: 0,
                        name: "foo".into(),
                        start: 0.0,
                        end: 50.0,
                        depth: 0,
                        parent: None,
                        self_value: 30.0,
                        kind: SpanKind::Event,
                        category: None,
                    },
                    Span {
                        id: 1,
                        name: "foo".into(),
                        start: 50.0,
                        end: 80.0,
                        depth: 0,
                        parent: None,
                        self_value: 20.0,
                        kind: SpanKind::Event,
                        category: None,
                    },
                    Span {
                        id: 2,
                        name: "bar".into(),
                        start: 10.0,
                        end: 40.0,
                        depth: 1,
                        parent: Some(0),
                        self_value: 30.0,
                        kind: SpanKind::Event,
                        category: None,
                    },
                ],
            }],
            frames: vec![],
            counters: vec![],
            async_spans: vec![],
            flow_arrows: vec![],
            markers: vec![],
            instant_events: vec![],
            object_events: vec![],
            cpu_samples: None,
        };

        let entries = get_ranked_entries(&profile, RankedSort::SelfTime, false);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "foo");
        assert_eq!(entries[0].self_time, 50.0);
        assert_eq!(entries[0].count, 2);
        assert_eq!(entries[1].name, "bar");

        let vp = Viewport {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 600.0,
            dpr: 1.0,
        };
        let cmds = render_ranked(&profile, &vp, RankedSort::SelfTime, false);
        let texts: Vec<_> = cmds
            .iter()
            .filter_map(|c| {
                if let RenderCommand::DrawText { text, .. } = c {
                    Some(text.clone())
                } else {
                    None
                }
            })
            .collect();
        assert!(texts.contains(&SharedStr::from("foo")));
        assert!(texts.contains(&SharedStr::from("bar")));
    }
}
