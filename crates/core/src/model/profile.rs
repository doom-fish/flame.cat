use flame_cat_protocol::{
    ProfileMeta, SharedStr, SourceFormat, Span, SpanCategory, SpanKind, ThreadGroup, TimeDomain,
    ValueUnit, VisualProfile,
};
use serde::{Deserialize, Serialize};

/// A single stack frame span in the profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Frame {
    /// Unique identifier within this profile.
    pub id: u64,
    /// Display name (function, component, etc.).
    pub name: String,
    /// Start time in microseconds from profile start.
    pub start: f64,
    /// End time in microseconds from profile start.
    pub end: f64,
    /// Stack depth (0 = top-level).
    pub depth: u32,
    /// Optional category for grouping / coloring.
    pub category: Option<String>,
    /// Index of the parent frame, if any.
    pub parent: Option<u64>,
    /// Self time (exclusive of children).
    pub self_time: f64,
    /// Thread or group name (for multi-thread traces).
    pub thread: Option<String>,
}

impl Frame {
    pub fn duration(&self) -> f64 {
        self.end - self.start
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileMetadata {
    pub name: Option<String>,
    pub start_time: f64,
    pub end_time: f64,
    /// Source format identifier (e.g. "chrome", "firefox", "react").
    pub format: String,
    /// Clock domain metadata for cross-profile alignment.
    #[serde(default)]
    pub time_domain: Option<TimeDomain>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub metadata: ProfileMetadata,
    pub frames: Vec<Frame>,
}

impl Profile {
    pub fn duration(&self) -> f64 {
        self.metadata.end_time - self.metadata.start_time
    }

    /// Get a frame by its id.
    pub fn frame(&self, id: u64) -> Option<&Frame> {
        self.frames.iter().find(|f| f.id == id)
    }

    /// Direct children of the given frame (or top-level frames if `None`).
    pub fn children(&self, parent: Option<u64>) -> Vec<&Frame> {
        self.frames.iter().filter(|f| f.parent == parent).collect()
    }

    /// Convert this Profile into the canonical VisualProfile protocol.
    pub fn into_visual_profile(self) -> VisualProfile {
        let source_format = match self.metadata.format.as_str() {
            "chrome" => SourceFormat::ChromeTrace,
            "firefox" => SourceFormat::FirefoxGecko,
            "react" => SourceFormat::ReactDevTools,
            "cpuprofile" => SourceFormat::CpuProfile,
            "speedscope" => SourceFormat::Speedscope,
            "collapsed" => SourceFormat::CollapsedStacks,
            "pprof" => SourceFormat::Pprof,
            "tracy" => SourceFormat::Tracy,
            "pix" => SourceFormat::Pix,
            "ebpf" | "ebpf-perf" => SourceFormat::Ebpf,
            _ => SourceFormat::Unknown,
        };

        let value_unit = match &source_format {
            SourceFormat::CollapsedStacks | SourceFormat::Ebpf => ValueUnit::Samples,
            SourceFormat::Pprof => ValueUnit::Nanoseconds,
            _ => ValueUnit::Microseconds,
        };

        let span_kind = match &source_format {
            SourceFormat::CollapsedStacks | SourceFormat::Ebpf | SourceFormat::Pprof => {
                SpanKind::Sample
            }
            _ => SpanKind::Event,
        };

        // String interning caches — each unique string is allocated once as
        // an Arc<str> (via SharedStr), subsequent occurrences just bump the
        // reference count (zero-cost clone).
        let mut name_cache: std::collections::HashMap<String, SharedStr> =
            std::collections::HashMap::new();
        let mut cat_cache: std::collections::HashMap<String, SharedStr> =
            std::collections::HashMap::new();
        let mut thread_cache: std::collections::HashMap<String, SharedStr> =
            std::collections::HashMap::new();

        // Group frames by thread name
        let mut thread_groups: std::collections::BTreeMap<SharedStr, Vec<Span>> =
            std::collections::BTreeMap::new();

        for f in self.frames {
            let name = name_cache
                .entry(f.name)
                .or_insert_with_key(|k| SharedStr::from(k.as_str()))
                .clone();

            let category = f.category.map(|c| {
                let cat_name = cat_cache
                    .entry(c)
                    .or_insert_with_key(|k| SharedStr::from(k.as_str()))
                    .clone();
                SpanCategory {
                    name: cat_name,
                    source: None,
                }
            });

            let thread_name = {
                let raw = f.thread.unwrap_or_else(|| "Main".to_string());
                thread_cache
                    .entry(raw)
                    .or_insert_with_key(|k| SharedStr::from(k.as_str()))
                    .clone()
            };

            let span = Span {
                id: f.id,
                name,
                start: f.start,
                end: f.end,
                depth: f.depth,
                parent: f.parent,
                self_value: f.self_time,
                kind: span_kind,
                category,
            };

            thread_groups.entry(thread_name).or_default().push(span);
        }

        // Sort thread groups: put "CrRendererMain" or "Main" first, then by event count
        let mut threads: Vec<ThreadGroup> = thread_groups
            .into_iter()
            .enumerate()
            .map(|(i, (name, spans))| ThreadGroup {
                id: i as u32,
                name: name.clone(),
                sort_key: thread_sort_key(&name),
                spans,
            })
            .collect();
        threads.sort_by_key(|t| t.sort_key);

        VisualProfile {
            meta: ProfileMeta {
                name: self.metadata.name.map(SharedStr::from),
                source_format,
                value_unit,
                total_value: self.metadata.end_time - self.metadata.start_time,
                start_time: self.metadata.start_time,
                end_time: self.metadata.end_time,
                time_domain: self.metadata.time_domain,
            },
            threads,
            frames: vec![],
        }
    }
}

/// Assign priority for thread sorting: main threads first, then by name.
fn thread_sort_key(name: &str) -> i64 {
    match name {
        "CrRendererMain" => 0,
        "Main" => 1,
        n if n.contains("Main") => 2,
        "Compositor" => 10,
        n if n.contains("Worker") => 20,
        n if n.contains("IO") => 30,
        _ => 50,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_profile(format: &str) -> Profile {
        Profile {
            metadata: ProfileMetadata {
                name: Some("test.json".into()),
                start_time: 0.0,
                end_time: 200.0,
                format: format.to_string(),
                time_domain: None,
            },
            frames: vec![
                Frame {
                    id: 0,
                    name: "main".into(),
                    start: 0.0,
                    end: 200.0,
                    depth: 0,
                    category: Some("js".into()),
                    parent: None,
                    self_time: 80.0,
                    thread: None,
                },
                Frame {
                    id: 1,
                    name: "render".into(),
                    start: 10.0,
                    end: 130.0,
                    depth: 1,
                    category: None,
                    parent: Some(0),
                    self_time: 120.0,
                    thread: None,
                },
            ],
        }
    }

    #[test]
    fn conversion_preserves_spans() {
        let vp = sample_profile("chrome").into_visual_profile();
        assert_eq!(vp.span_count(), 2);
        assert_eq!(vp.threads.len(), 1);
        assert_eq!(vp.threads[0].name, "Main");

        let root = vp.span(0).expect("span 0 must exist");
        assert_eq!(root.name, "main");
        assert!((root.self_value - 80.0).abs() < f64::EPSILON);
        assert!(root.category.is_some());
        assert_eq!(root.category.as_ref().expect("category").name, "js");
    }

    #[test]
    fn conversion_maps_source_format() {
        for (fmt, expected) in [
            ("chrome", SourceFormat::ChromeTrace),
            ("firefox", SourceFormat::FirefoxGecko),
            ("react", SourceFormat::ReactDevTools),
            ("collapsed", SourceFormat::CollapsedStacks),
            ("pprof", SourceFormat::Pprof),
            ("tracy", SourceFormat::Tracy),
            ("ebpf", SourceFormat::Ebpf),
            ("unknown-fmt", SourceFormat::Unknown),
        ] {
            let vp = sample_profile(fmt).into_visual_profile();
            assert_eq!(vp.meta.source_format, expected, "format: {fmt}");
        }
    }

    #[test]
    fn conversion_sets_value_unit() {
        let collapsed = sample_profile("collapsed").into_visual_profile();
        assert_eq!(collapsed.meta.value_unit, ValueUnit::Samples);

        let chrome = sample_profile("chrome").into_visual_profile();
        assert_eq!(chrome.meta.value_unit, ValueUnit::Microseconds);

        let pprof = sample_profile("pprof").into_visual_profile();
        assert_eq!(pprof.meta.value_unit, ValueUnit::Nanoseconds);
    }

    #[test]
    fn conversion_sets_span_kind() {
        let event_profile = sample_profile("chrome").into_visual_profile();
        assert_eq!(
            event_profile
                .all_spans()
                .next()
                .expect("must have spans")
                .kind,
            SpanKind::Event
        );

        let sampled = sample_profile("collapsed").into_visual_profile();
        assert_eq!(
            sampled.all_spans().next().expect("must have spans").kind,
            SpanKind::Sample
        );
    }

    #[test]
    fn conversion_preserves_metadata() {
        let vp = sample_profile("chrome").into_visual_profile();
        assert_eq!(vp.meta.name, Some("test.json".into()));
        assert!((vp.meta.total_value - 200.0).abs() < f64::EPSILON);
        assert!((vp.duration() - 200.0).abs() < f64::EPSILON);
    }

    #[test]
    fn conversion_preserves_parent_pointers() {
        let vp = sample_profile("chrome").into_visual_profile();
        let children = vp.children(Some(0));
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].name, "render");
    }
}
