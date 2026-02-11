use flame_cat_protocol::VisualProfile;
use serde::{Deserialize, Serialize};

/// A profiling entry within a session — one loaded profile with alignment data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileEntry {
    /// The parsed visual profile.
    pub profile: VisualProfile,
    /// Offset in µs to apply to all timestamps when mapping to the unified
    /// session timeline. Computed from clock domain alignment.
    pub offset_us: f64,
    /// Human-readable label for this profile source.
    pub label: String,
}

impl ProfileEntry {
    /// Map a timestamp from this profile's local time to the unified session
    /// timeline, applying the offset and unit normalization.
    pub fn to_session_time(&self, local_time: f64) -> f64 {
        let factor = self
            .profile
            .meta
            .value_unit
            .to_microseconds_factor()
            .unwrap_or(1.0);
        local_time * factor + self.offset_us
    }

    /// Start time on the unified session timeline (µs).
    pub fn session_start(&self) -> f64 {
        self.to_session_time(self.profile.meta.start_time)
    }

    /// End time on the unified session timeline (µs).
    pub fn session_end(&self) -> f64 {
        self.to_session_time(self.profile.meta.end_time)
    }
}

/// Multi-profile session container.
///
/// Manages one or more profiles on a unified timeline. Profiles that share
/// a compatible clock domain (e.g. both `CLOCK_MONOTONIC`) are automatically
/// aligned; others can be manually offset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    profiles: Vec<ProfileEntry>,
}

impl Session {
    /// Create a new empty session.
    pub fn new() -> Self {
        Self {
            profiles: Vec::new(),
        }
    }

    /// Create a session from a single profile (most common case).
    pub fn from_profile(profile: VisualProfile, label: impl Into<String>) -> Self {
        let mut session = Self::new();
        session.add_profile(profile, label);
        session
    }

    /// Add a profile to the session.
    ///
    /// Computes offset automatically if the new profile shares a compatible
    /// clock domain with existing profiles. Otherwise offset is 0 (manual
    /// alignment required).
    pub fn add_profile(&mut self, profile: VisualProfile, label: impl Into<String>) {
        let offset_us = self.compute_offset(&profile);
        self.profiles.push(ProfileEntry {
            profile,
            offset_us,
            label: label.into(),
        });
    }

    /// All profile entries in the session.
    pub fn profiles(&self) -> &[ProfileEntry] {
        &self.profiles
    }

    /// Mutable access to profile entries (for manual offset adjustment).
    pub fn profiles_mut(&mut self) -> &mut [ProfileEntry] {
        &mut self.profiles
    }

    /// Number of profiles in the session.
    pub fn len(&self) -> usize {
        self.profiles.len()
    }

    /// Whether the session has no profiles.
    pub fn is_empty(&self) -> bool {
        self.profiles.is_empty()
    }

    /// Unified start time across all profiles (µs).
    pub fn start_time(&self) -> f64 {
        self.profiles
            .iter()
            .map(ProfileEntry::session_start)
            .fold(f64::INFINITY, f64::min)
    }

    /// Unified end time across all profiles (µs).
    pub fn end_time(&self) -> f64 {
        self.profiles
            .iter()
            .map(ProfileEntry::session_end)
            .fold(f64::NEG_INFINITY, f64::max)
    }

    /// Total duration of the session (µs).
    pub fn duration(&self) -> f64 {
        let start = self.start_time();
        let end = self.end_time();
        if start.is_finite() && end.is_finite() {
            end - start
        } else {
            0.0
        }
    }

    /// Compute the offset for a new profile based on clock domain compatibility.
    ///
    /// If the new profile and an existing profile both have time domains
    /// on the same clock (e.g. both `CLOCK_MONOTONIC`), timestamps are
    /// already comparable — the offset accounts only for unit differences.
    ///
    /// If clocks are incompatible or unknown, offset is 0.0 (profiles
    /// overlap at their raw timestamp positions until manually adjusted).
    fn compute_offset(&self, profile: &VisualProfile) -> f64 {
        if self.profiles.is_empty() {
            return 0.0;
        }

        let Some(new_td) = &profile.meta.time_domain else {
            return 0.0;
        };

        // Find first existing profile with a compatible time domain.
        for existing in &self.profiles {
            if let Some(ref existing_td) = existing.profile.meta.time_domain
                && new_td.is_compatible(existing_td)
            {
                return 0.0;
            }
        }

        0.0
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flame_cat_protocol::{
        ClockKind, ProfileMeta, SourceFormat, Span, SpanKind, ThreadGroup, TimeDomain, ValueUnit,
    };

    fn make_profile(
        start: f64,
        end: f64,
        unit: ValueUnit,
        time_domain: Option<TimeDomain>,
    ) -> VisualProfile {
        VisualProfile {
            meta: ProfileMeta {
                name: Some("test".into()),
                source_format: SourceFormat::ChromeTrace,
                value_unit: unit,
                total_value: end - start,
                start_time: start,
                end_time: end,
                time_domain,
            },
            threads: vec![ThreadGroup {
                id: 0,
                name: "Main".into(),
                sort_key: 0,
                spans: vec![Span {
                    id: 0,
                    name: "root".into(),
                    start,
                    end,
                    depth: 0,
                    parent: None,
                    self_value: end - start,
                    kind: SpanKind::Event,
                    category: None,
                }],
            }],
            frames: vec![],
        }
    }

    #[test]
    fn single_profile_session() {
        let profile = make_profile(100.0, 200.0, ValueUnit::Microseconds, None);
        let session = Session::from_profile(profile, "test.json");
        assert_eq!(session.len(), 1);
        assert!((session.start_time() - 100.0).abs() < f64::EPSILON);
        assert!((session.end_time() - 200.0).abs() < f64::EPSILON);
        assert!((session.duration() - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn multi_profile_expands_range() {
        let p1 = make_profile(100.0, 200.0, ValueUnit::Microseconds, None);
        let p2 = make_profile(300.0, 500.0, ValueUnit::Microseconds, None);
        let mut session = Session::from_profile(p1, "p1");
        session.add_profile(p2, "p2");
        assert_eq!(session.len(), 2);
        assert!((session.start_time() - 100.0).abs() < f64::EPSILON);
        assert!((session.end_time() - 500.0).abs() < f64::EPSILON);
    }

    #[test]
    fn compatible_clock_domains() {
        let td_mono = TimeDomain {
            clock_kind: ClockKind::LinuxMonotonic,
            origin_label: None,
        };
        let td_perf_now = TimeDomain {
            clock_kind: ClockKind::PerformanceNow,
            origin_label: None,
        };
        assert!(td_mono.is_compatible(&td_perf_now));
        assert!(td_perf_now.is_compatible(&td_mono));

        let td_wall = TimeDomain {
            clock_kind: ClockKind::WallClock,
            origin_label: None,
        };
        assert!(!td_mono.is_compatible(&td_wall));
    }

    #[test]
    fn unit_normalization_in_session_time() {
        // Profile with nanosecond timestamps (like perf/eBPF).
        let profile = make_profile(
            1_000_000.0,  // 1ms in ns
            10_000_000.0, // 10ms in ns
            ValueUnit::Nanoseconds,
            None,
        );
        let session = Session::from_profile(profile, "perf");
        let entry = &session.profiles()[0];

        // to_session_time should convert ns → µs (factor 0.001)
        let session_start = entry.to_session_time(1_000_000.0);
        assert!((session_start - 1_000.0).abs() < f64::EPSILON); // 1ms = 1000µs
    }

    #[test]
    fn empty_session() {
        let session = Session::new();
        assert!(session.is_empty());
        assert_eq!(session.duration(), 0.0);
    }
}
