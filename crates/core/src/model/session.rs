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
    /// Four cases:
    /// 1. Same clock (e.g. both `LinuxMonotonic`): offset=0, unit normalization
    ///    is enough.
    /// 2. `PerformanceNow` ↔ `LinuxMonotonic` with `navigation_start_us`:
    ///    exact alignment — `monotonic = navigation_start + performance.now()`.
    /// 3. `PerformanceNow` ↔ `LinuxMonotonic` without `navigation_start_us`:
    ///    fall through to case 4.
    /// 4. No time domain or incompatible clocks: align start to session start.
    fn compute_offset(&self, profile: &VisualProfile) -> f64 {
        use flame_cat_protocol::ClockKind;

        if self.profiles.is_empty() {
            return 0.0;
        }

        if let Some(ref new_td) = profile.meta.time_domain {
            for existing in &self.profiles {
                let Some(ref existing_td) = existing.profile.meta.time_domain else {
                    continue;
                };
                if !new_td.is_compatible(existing_td) {
                    continue;
                }

                // Same clock kind: directly comparable.
                if new_td.clock_kind == existing_td.clock_kind {
                    return 0.0;
                }

                // PerformanceNow ↔ LinuxMonotonic: use navigationStart anchor.
                // navigationStart is the monotonic-clock µs at which performance.now() == 0.
                let nav_start = existing_td
                    .navigation_start_us
                    .or(new_td.navigation_start_us);
                if let Some(nav_start_us) = nav_start {
                    if new_td.clock_kind == ClockKind::PerformanceNow {
                        // New profile uses performance.now(): session_time = nav_start + local_time_µs
                        return nav_start_us;
                    }
                    // New profile is LinuxMonotonic, existing is PerformanceNow (rare)
                    return 0.0;
                }

                // Compatible but no navigationStart — fall through.
                break;
            }
        }

        // No compatible clock found (or no time domain at all).
        // Align new profile's start to the existing session's start.
        let session_start = self.start_time();
        let new_factor = profile
            .meta
            .value_unit
            .to_microseconds_factor()
            .unwrap_or(1.0);
        let new_start_us = profile.meta.start_time * new_factor;
        session_start - new_start_us
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
            counters: vec![],
            async_spans: vec![],
            flow_arrows: vec![],
            markers: vec![],
            instant_events: vec![],
            object_events: vec![],
            cpu_samples: None,
            network_requests: vec![],
            screenshots: vec![],
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
    fn multi_profile_auto_aligns_no_time_domain() {
        let p1 = make_profile(100.0, 200.0, ValueUnit::Microseconds, None);
        let p2 = make_profile(300.0, 500.0, ValueUnit::Microseconds, None);
        let mut session = Session::from_profile(p1, "p1");
        session.add_profile(p2, "p2");
        assert_eq!(session.len(), 2);
        // p2 is auto-aligned: offset = 100 - 300 = -200
        // p2 session range: 300 + (-200) = 100, 500 + (-200) = 300
        assert!((session.start_time() - 100.0).abs() < f64::EPSILON);
        assert!((session.end_time() - 300.0).abs() < f64::EPSILON);
    }

    #[test]
    fn compatible_clock_domains() {
        let td_mono = TimeDomain {
            clock_kind: ClockKind::LinuxMonotonic,
            origin_label: None,
            navigation_start_us: None,
        };
        let td_perf_now = TimeDomain {
            clock_kind: ClockKind::PerformanceNow,
            origin_label: None,
            navigation_start_us: None,
        };
        assert!(td_mono.is_compatible(&td_perf_now));
        assert!(td_perf_now.is_compatible(&td_mono));

        let td_wall = TimeDomain {
            clock_kind: ClockKind::WallClock,
            origin_label: None,
            navigation_start_us: None,
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

    #[test]
    fn auto_align_relative_onto_absolute() {
        // Chrome trace with absolute monotonic timestamps (µs)
        let chrome = make_profile(
            325_186_766_678.0,
            325_191_926_889.0,
            ValueUnit::Microseconds,
            Some(TimeDomain {
                clock_kind: ClockKind::LinuxMonotonic,
                origin_label: None,
                navigation_start_us: None,
            }),
        );
        // Profile with no time domain (unknown source)
        let unknown = make_profile(2836.0, 2846.0, ValueUnit::Microseconds, None);

        let mut session = Session::from_profile(chrome, "chrome");
        session.add_profile(unknown, "unknown");

        // Unknown profile should be aligned to Chrome's start
        let unknown_entry = &session.profiles()[1];
        let expected_offset = 325_186_766_678.0 - 2836.0;
        assert!(
            (unknown_entry.offset_us - expected_offset).abs() < 1.0,
            "Unknown offset should align to Chrome start: got {}, expected {}",
            unknown_entry.offset_us,
            expected_offset,
        );
        assert!(
            (unknown_entry.session_start() - 325_186_766_678.0).abs() < 1.0,
            "Unknown session start should equal Chrome start",
        );
    }

    #[test]
    fn auto_align_performance_now_with_navigation_start() {
        // Chrome trace with navigationStart anchor
        let chrome = make_profile(
            325_186_766_678.0,
            325_191_926_889.0,
            ValueUnit::Microseconds,
            Some(TimeDomain {
                clock_kind: ClockKind::LinuxMonotonic,
                origin_label: None,
                navigation_start_us: Some(325_186_769_518.0),
            }),
        );
        // React DevTools with PerformanceNow timestamps (µs)
        let react = make_profile(
            2_836_400.0, // performance.now() = 2836.4ms = 2836400µs
            2_845_900.0,
            ValueUnit::Microseconds,
            Some(TimeDomain {
                clock_kind: ClockKind::PerformanceNow,
                origin_label: None,
                navigation_start_us: None,
            }),
        );

        let mut session = Session::from_profile(chrome, "chrome");
        session.add_profile(react, "react");

        // React offset should be navigationStart
        let react_entry = &session.profiles()[1];
        assert!(
            (react_entry.offset_us - 325_186_769_518.0).abs() < 1.0,
            "React offset should be navigationStart: got {}, expected 325186769518",
            react_entry.offset_us,
        );
        // React session start = navigationStart + 2836400µs
        let expected_start = 325_186_769_518.0 + 2_836_400.0;
        assert!(
            (react_entry.session_start() - expected_start).abs() < 1.0,
            "React start should be nav_start + perf_now: got {:.0}, expected {:.0}",
            react_entry.session_start(),
            expected_start,
        );
    }
}
