use serde::{Deserialize, Serialize};

/// The canonical visual profile IR that every profiling format compiles into.
///
/// This is the single intermediate representation between format-specific
/// parsers and format-agnostic view transforms / renderers.
///
/// ```text
///   Chrome ─┐
///   Firefox ├─▶ VisualProfile ──▶ View Transform ──▶ RenderCommand[] ──▶ Renderer
///   pprof  ─┤       (this)          (time order,       (DrawRect,        (WebGPU,
///   Tracy  ─┤                        left heavy,        DrawText,         Canvas,
///   eBPF   ─┘                        ranked…)           SetClip…)         SVG…)
/// ```
///
/// # Design principles
///
/// 1. **Format-agnostic** — No Chrome-isms, no pprof-isms. Any profiler output
///    can be normalized into this representation.
/// 2. **Semantically rich** — Carries enough metadata (units, span kinds,
///    thread structure) for all visualization modes.
/// 3. **Serializable** — Can be saved to disk, sent over the wire, or
///    passed through WASM boundaries as JSON.
/// 4. **Flat spans with tree pointers** — Spans carry parent references and
///    depth, enabling both tree traversal and flat iteration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualProfile {
    pub meta: ProfileMeta,
    pub threads: Vec<ThreadGroup>,
}

/// Top-level metadata about the profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileMeta {
    /// Human-readable name (filename, app name, etc.).
    pub name: Option<String>,
    /// Source format (for display, not for branching logic).
    pub source_format: SourceFormat,
    /// What the span values represent.
    pub value_unit: ValueUnit,
    /// Total duration/weight of the profile in the unit specified by `value_unit`.
    pub total_value: f64,
    /// Wall-clock start time (microseconds since epoch), if known.
    pub start_time: f64,
    /// Wall-clock end time (microseconds since epoch), if known.
    pub end_time: f64,
}

/// The original profiling format — informational only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceFormat {
    ChromeTrace,
    FirefoxGecko,
    ReactDevTools,
    CpuProfile,
    Speedscope,
    CollapsedStacks,
    Pprof,
    Tracy,
    Pix,
    Ebpf,
    Unknown,
}

impl std::fmt::Display for SourceFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ChromeTrace => write!(f, "Chrome Trace"),
            Self::FirefoxGecko => write!(f, "Firefox Gecko"),
            Self::ReactDevTools => write!(f, "React DevTools"),
            Self::CpuProfile => write!(f, "V8 CPU Profile"),
            Self::Speedscope => write!(f, "Speedscope"),
            Self::CollapsedStacks => write!(f, "Collapsed Stacks"),
            Self::Pprof => write!(f, "pprof"),
            Self::Tracy => write!(f, "Tracy"),
            Self::Pix => write!(f, "PIX"),
            Self::Ebpf => write!(f, "eBPF"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

/// What the numerical values in spans represent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValueUnit {
    /// Wall-clock time in microseconds.
    Microseconds,
    /// Wall-clock time in milliseconds.
    Milliseconds,
    /// Wall-clock time in nanoseconds.
    Nanoseconds,
    /// CPU sample count (perf, collapsed stacks).
    Samples,
    /// Memory in bytes.
    Bytes,
    /// Arbitrary weight (custom profilers).
    Weight,
}

impl ValueUnit {
    /// Format a value in this unit for display.
    pub fn format_value(&self, value: f64) -> String {
        match self {
            Self::Microseconds => {
                if value >= 1_000_000.0 {
                    format!("{:.2}s", value / 1_000_000.0)
                } else if value >= 1_000.0 {
                    format!("{:.1}ms", value / 1_000.0)
                } else {
                    format!("{:.0}µs", value)
                }
            }
            Self::Milliseconds => {
                if value >= 1_000.0 {
                    format!("{:.2}s", value / 1_000.0)
                } else {
                    format!("{:.1}ms", value)
                }
            }
            Self::Nanoseconds => {
                if value >= 1_000_000_000.0 {
                    format!("{:.2}s", value / 1_000_000_000.0)
                } else if value >= 1_000_000.0 {
                    format!("{:.1}ms", value / 1_000_000.0)
                } else if value >= 1_000.0 {
                    format!("{:.0}µs", value / 1_000.0)
                } else {
                    format!("{:.0}ns", value)
                }
            }
            Self::Samples => format!("{} samples", value as u64),
            Self::Bytes => {
                if value >= 1_073_741_824.0 {
                    format!("{:.1} GiB", value / 1_073_741_824.0)
                } else if value >= 1_048_576.0 {
                    format!("{:.1} MiB", value / 1_048_576.0)
                } else if value >= 1_024.0 {
                    format!("{:.1} KiB", value / 1_024.0)
                } else {
                    format!("{} B", value as u64)
                }
            }
            Self::Weight => format!("{:.0}", value),
        }
    }
}

/// A logical grouping of spans (thread, process, GPU queue, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadGroup {
    /// Unique id within this profile.
    pub id: u32,
    /// Display name ("Main Thread", "Renderer", "GC", etc.).
    pub name: String,
    /// Process/thread identifiers from the source format.
    pub sort_key: i64,
    /// All spans in this thread, ordered by start time.
    pub spans: Vec<Span>,
}

/// A single visual span — the atomic unit of the visual profile.
///
/// Replaces the old `Frame` struct. Every span has:
/// - A time/value range (start → end)
/// - A depth in the call stack
/// - A parent pointer for tree navigation
/// - A kind (event with known duration, or a sample with a weight)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Span {
    /// Unique id within this profile.
    pub id: u64,
    /// Display name (function name, component, zone, etc.).
    pub name: String,
    /// Start position in the profile's value unit.
    pub start: f64,
    /// End position in the profile's value unit.
    pub end: f64,
    /// Stack depth (0 = top-level root).
    pub depth: u32,
    /// Parent span id, if any.
    pub parent: Option<u64>,
    /// Self value (exclusive of children), in the profile's value unit.
    pub self_value: f64,
    /// How this span was produced.
    pub kind: SpanKind,
    /// Optional semantic category for grouping and coloring.
    pub category: Option<SpanCategory>,
}

impl Span {
    /// Total duration/value of this span.
    pub fn duration(&self) -> f64 {
        self.end - self.start
    }
}

/// How a span was produced — affects how views interpret it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpanKind {
    /// Span with known start/end times (Chrome X/B/E events, Tracy zones).
    Event,
    /// Span reconstructed from sampling data (perf, pprof, eBPF).
    Sample,
    /// Synthetic span (aggregated, merged, or generated by a view transform).
    Synthetic,
}

/// Semantic categories for coloring and grouping.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanCategory {
    /// Category name ("js", "gc", "layout", "paint", "gpu", "react", etc.).
    pub name: String,
    /// Optional source location (file path, module name).
    pub source: Option<String>,
}

// --- Conversions from the old Profile model ---

impl VisualProfile {
    /// Total duration of the profile.
    pub fn duration(&self) -> f64 {
        self.meta.end_time - self.meta.start_time
    }

    /// Get a span by id, searching all threads.
    pub fn span(&self, id: u64) -> Option<&Span> {
        self.threads
            .iter()
            .flat_map(|t| &t.spans)
            .find(|s| s.id == id)
    }

    /// Iterate all spans across all threads.
    pub fn all_spans(&self) -> impl Iterator<Item = &Span> {
        self.threads.iter().flat_map(|t| &t.spans)
    }

    /// Total number of spans across all threads.
    pub fn span_count(&self) -> usize {
        self.threads.iter().map(|t| t.spans.len()).sum()
    }

    /// Direct children of the given span (or top-level spans if `None`),
    /// searching across all threads.
    pub fn children(&self, parent: Option<u64>) -> Vec<&Span> {
        self.all_spans().filter(|s| s.parent == parent).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_profile() -> VisualProfile {
        VisualProfile {
            meta: ProfileMeta {
                name: Some("test".into()),
                source_format: SourceFormat::ChromeTrace,
                value_unit: ValueUnit::Microseconds,
                total_value: 100.0,
                start_time: 0.0,
                end_time: 100.0,
            },
            threads: vec![
                ThreadGroup {
                    id: 0,
                    name: "Main".into(),
                    sort_key: 0,
                    spans: vec![
                        Span {
                            id: 0,
                            name: "root".into(),
                            start: 0.0,
                            end: 100.0,
                            depth: 0,
                            parent: None,
                            self_value: 40.0,
                            kind: SpanKind::Event,
                            category: None,
                        },
                        Span {
                            id: 1,
                            name: "child".into(),
                            start: 10.0,
                            end: 70.0,
                            depth: 1,
                            parent: Some(0),
                            self_value: 60.0,
                            kind: SpanKind::Event,
                            category: Some(SpanCategory {
                                name: "js".into(),
                                source: None,
                            }),
                        },
                    ],
                },
                ThreadGroup {
                    id: 1,
                    name: "Worker".into(),
                    sort_key: 1,
                    spans: vec![Span {
                        id: 2,
                        name: "task".into(),
                        start: 20.0,
                        end: 50.0,
                        depth: 0,
                        parent: None,
                        self_value: 30.0,
                        kind: SpanKind::Event,
                        category: None,
                    }],
                },
            ],
        }
    }

    #[test]
    fn duration() {
        let p = sample_profile();
        assert!((p.duration() - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn span_count_across_threads() {
        let p = sample_profile();
        assert_eq!(p.span_count(), 3);
    }

    #[test]
    fn span_lookup_by_id() {
        let p = sample_profile();
        assert_eq!(p.span(0).map(|s| &s.name[..]), Some("root"));
        assert_eq!(p.span(2).map(|s| &s.name[..]), Some("task"));
        assert!(p.span(99).is_none());
    }

    #[test]
    fn children_of_root() {
        let p = sample_profile();
        let kids = p.children(Some(0));
        assert_eq!(kids.len(), 1);
        assert_eq!(kids[0].name, "child");
    }

    #[test]
    fn top_level_spans() {
        let p = sample_profile();
        let roots = p.children(None);
        assert_eq!(roots.len(), 2);
    }

    #[test]
    fn all_spans_iterates_across_threads() {
        let p = sample_profile();
        let names: Vec<_> = p.all_spans().map(|s| &s.name[..]).collect();
        assert_eq!(names, vec!["root", "child", "task"]);
    }

    #[test]
    fn span_duration() {
        let s = Span {
            id: 0,
            name: "x".into(),
            start: 10.0,
            end: 30.0,
            depth: 0,
            parent: None,
            self_value: 20.0,
            kind: SpanKind::Event,
            category: None,
        };
        assert!((s.duration() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn value_unit_format_microseconds() {
        assert_eq!(ValueUnit::Microseconds.format_value(500.0), "500µs");
        assert_eq!(ValueUnit::Microseconds.format_value(1500.0), "1.5ms");
        assert_eq!(ValueUnit::Microseconds.format_value(2_500_000.0), "2.50s");
    }

    #[test]
    fn value_unit_format_samples() {
        assert_eq!(ValueUnit::Samples.format_value(42.0), "42 samples");
    }

    #[test]
    fn value_unit_format_bytes() {
        assert_eq!(ValueUnit::Bytes.format_value(512.0), "512 B");
        assert_eq!(ValueUnit::Bytes.format_value(2048.0), "2.0 KiB");
        assert_eq!(ValueUnit::Bytes.format_value(5_242_880.0), "5.0 MiB");
    }

    #[test]
    fn source_format_display() {
        assert_eq!(SourceFormat::ChromeTrace.to_string(), "Chrome Trace");
        assert_eq!(SourceFormat::Ebpf.to_string(), "eBPF");
        assert_eq!(SourceFormat::Unknown.to_string(), "Unknown");
    }

    #[test]
    fn serialization_roundtrip() {
        let p = sample_profile();
        let json = serde_json::to_string(&p).expect("serialize");
        let p2: VisualProfile = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(p2.span_count(), 3);
        assert_eq!(p2.meta.source_format, SourceFormat::ChromeTrace);
    }
}
