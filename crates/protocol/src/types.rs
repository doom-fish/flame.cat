use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }
}

/// Describes the clock source used by a profiling tool.
///
/// Used to determine whether two profiles can be automatically aligned
/// on a shared timeline (same clock = timestamps directly comparable
/// after unit normalization).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClockKind {
    /// Linux `CLOCK_MONOTONIC` — shared across all processes on the same
    /// machine. Used by Chrome traces (on Linux), perf, eBPF, Tracy.
    LinuxMonotonic,
    /// Browser `performance.now()` — monotonic within a page, origin at
    /// navigation start. Same underlying clock as `CLOCK_MONOTONIC` on
    /// Linux, but with a page-specific offset.
    PerformanceNow,
    /// `CLOCK_REALTIME` / wall-clock time (e.g. pprof `time_nanos`).
    /// Subject to NTP adjustments — less reliable for alignment.
    WallClock,
    /// Per-thread CPU time (`CLOCK_THREAD_CPUTIME_ID`).
    CpuTime,
    /// No real time axis — sample counts or weights only.
    Samples,
    /// Clock source could not be determined from the data.
    Unknown,
}

/// Metadata describing the time domain of a profile.
///
/// Enables automatic alignment: two profiles with the same `ClockKind`
/// and overlapping time ranges can be placed on a shared timeline by
/// normalizing their units.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimeDomain {
    /// Which clock produced the timestamps.
    pub clock_kind: ClockKind,
    /// Optional label identifying the clock origin (e.g. "Chrome PID 12345").
    pub origin_label: Option<String>,
}

impl TimeDomain {
    /// Whether two time domains share the same underlying clock and can
    /// be automatically aligned after unit normalization.
    pub fn is_compatible(&self, other: &TimeDomain) -> bool {
        matches!(
            (&self.clock_kind, &other.clock_kind),
            (ClockKind::LinuxMonotonic, ClockKind::LinuxMonotonic)
                | (ClockKind::PerformanceNow, ClockKind::PerformanceNow)
                | (ClockKind::WallClock, ClockKind::WallClock)
                | (ClockKind::LinuxMonotonic, ClockKind::PerformanceNow)
                | (ClockKind::PerformanceNow, ClockKind::LinuxMonotonic)
        )
    }
}
