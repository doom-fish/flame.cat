pub mod chrome;
pub mod collapsed;
pub mod cpuprofile;
pub mod ebpf;
pub mod firefox;
pub mod pix;
pub mod pprof;
pub mod react;
pub mod speedscope;
pub mod tracy;

use crate::model::Profile;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("chrome: {0}")]
    Chrome(#[from] chrome::ChromeParseError),
    #[error("react: {0}")]
    React(#[from] react::ReactParseError),
    #[error("collapsed: {0}")]
    Collapsed(#[from] collapsed::CollapsedParseError),
    #[error("cpuprofile: {0}")]
    CpuProfile(#[from] cpuprofile::CpuProfileParseError),
    #[error("speedscope: {0}")]
    Speedscope(#[from] speedscope::SpeedscopeParseError),
    #[error("firefox: {0}")]
    Firefox(#[from] firefox::FirefoxParseError),
    #[error("tracy: {0}")]
    Tracy(#[from] tracy::TracyParseError),
    #[error("pix: {0}")]
    Pix(#[from] pix::PixParseError),
    #[error("pprof: {0}")]
    Pprof(#[from] pprof::PprofParseError),
    #[error("ebpf: {0}")]
    Ebpf(#[from] ebpf::EbpfParseError),
    #[error("unable to detect format")]
    UnknownFormat,
}

/// Auto-detect the profile format and parse it.
///
/// Detection strategy:
/// 1. Try to parse as JSON first (most formats are JSON-based).
/// 2. Inspect top-level keys to identify the format.
/// 3. Fall back to text-based formats (collapsed stacks, perf script, bpftrace).
pub fn parse_auto(data: &[u8]) -> Result<Profile, ParseError> {
    // Try JSON-based formats first.
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(data) {
        if let Some(obj) = value.as_object() {
            // Speedscope: has "$schema" containing "speedscope" or has "shared" + "profiles"
            if obj.contains_key("$schema")
                && obj["$schema"]
                    .as_str()
                    .is_some_and(|s| s.contains("speedscope"))
            {
                return Ok(speedscope::parse_speedscope(data)?);
            }
            if obj.contains_key("shared") && obj.contains_key("profiles") {
                return Ok(speedscope::parse_speedscope(data)?);
            }

            // React DevTools: has "dataForRoots"
            if obj.contains_key("dataForRoots") {
                return Ok(react::parse_react_profile(data)?);
            }

            // Tracy: has "threads" with "zones"
            if let Some(threads) = obj.get("threads").and_then(|v| v.as_array())
                && threads.iter().any(|t| t.get("zones").is_some())
            {
                return Ok(tracy::parse_tracy(data)?);
            }

            // Firefox Gecko: has "threads" array with stackTable/frameTable
            if let Some(threads) = obj.get("threads").and_then(|v| v.as_array())
                && threads
                    .iter()
                    .any(|t| t.get("stackTable").is_some() || t.get("frameTable").is_some())
            {
                return Ok(firefox::parse_firefox(data)?);
            }

            // PIX: has "events" array with objects containing "start"
            if let Some(events) = obj.get("events").and_then(|v| v.as_array())
                && events.iter().any(|e| e.get("start").is_some())
            {
                return Ok(pix::parse_pix(data)?);
            }

            // pprof JSON: has "samples" + "locations" + "functions"
            if obj.contains_key("samples")
                && obj.contains_key("locations")
                && obj.contains_key("functions")
            {
                return Ok(pprof::parse_pprof(data)?);
            }

            // V8 CPU profile: has "nodes" + "startTime" + "endTime"
            if obj.contains_key("nodes")
                && obj.contains_key("startTime")
                && obj.contains_key("endTime")
            {
                return Ok(cpuprofile::parse_cpuprofile(data)?);
            }

            // Chrome trace: has "traceEvents"
            if obj.contains_key("traceEvents") {
                return Ok(chrome::parse_chrome_trace(data)?);
            }
        }

        // Chrome trace array format: top-level JSON array with objects containing "ph"
        if let Some(arr) = value.as_array()
            && arr.iter().any(|v| v.get("ph").is_some())
        {
            return Ok(chrome::parse_chrome_trace(data)?);
        }
    }

    // Not JSON — try text-based formats.

    // eBPF bpftrace/perf script format
    if let Ok(text) = std::str::from_utf8(data)
        && (text.contains("@[")
            || text
                .lines()
                .any(|l| l.starts_with('\t') && l.trim().len() > 8))
        && let Ok(profile) = ebpf::parse_ebpf(data)
    {
        return Ok(profile);
    }

    // Collapsed/folded stacks (most permissive text format — try last)
    if let Ok(profile) = collapsed::parse_collapsed(data) {
        return Ok(profile);
    }

    Err(ParseError::UnknownFormat)
}
