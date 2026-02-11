use serde::Deserialize;
use thiserror::Error;

use crate::model::{Frame, Profile, ProfileMetadata};

#[derive(Debug, Error)]
pub enum FirefoxParseError {
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("no threads found")]
    NoThreads,
}

/// Firefox/Gecko profiler format top level.
#[derive(Debug, Deserialize)]
struct GeckoProfile {
    #[serde(default)]
    threads: Vec<GeckoThread>,
    #[serde(default)]
    meta: Option<GeckoMeta>,
}

#[derive(Debug, Deserialize)]
struct GeckoMeta {
    #[serde(default)]
    interval: Option<f64>,
    #[serde(default, rename = "startTime")]
    start_time: Option<f64>,
    #[serde(default)]
    product: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GeckoThread {
    #[serde(default)]
    name: Option<String>,
    #[serde(rename = "stackTable")]
    stack_table: Option<GeckoStackTable>,
    #[serde(rename = "frameTable")]
    frame_table: Option<GeckoFrameTable>,
    #[serde(rename = "stringTable")]
    string_table: Option<Vec<String>>,
    samples: Option<GeckoSamples>,
    #[serde(rename = "funcTable")]
    func_table: Option<GeckoFuncTable>,
}

#[derive(Debug, Deserialize)]
struct GeckoStackTable {
    frame: Vec<usize>,
    prefix: Vec<Option<usize>>,
}

#[derive(Debug, Deserialize)]
struct GeckoFrameTable {
    func: Vec<usize>,
    #[serde(default)]
    #[allow(dead_code)]
    category: Vec<Option<usize>>,
}

#[derive(Debug, Deserialize)]
struct GeckoFuncTable {
    name: Vec<usize>,
}

#[derive(Debug, Deserialize)]
struct GeckoSamples {
    stack: Vec<Option<usize>>,
    time: Vec<f64>,
}

/// Parse a Firefox/Gecko profiler JSON into a `Profile`.
///
/// Used by: Firefox DevTools profiler, `profiler.firefox.com`.
pub fn parse_firefox(data: &[u8]) -> Result<Profile, FirefoxParseError> {
    let gecko: GeckoProfile = serde_json::from_slice(data)?;

    if gecko.threads.is_empty() {
        return Err(FirefoxParseError::NoThreads);
    }

    let mut all_frames: Vec<Frame> = Vec::new();
    let mut next_id: u64 = 0;

    let profile_start = gecko
        .meta
        .as_ref()
        .and_then(|m| m.start_time)
        .unwrap_or(0.0);
    let interval = gecko.meta.as_ref().and_then(|m| m.interval).unwrap_or(1.0);

    for thread in &gecko.threads {
        let Some(stack_table) = &thread.stack_table else {
            continue;
        };
        let Some(frame_table) = &thread.frame_table else {
            continue;
        };
        let Some(string_table) = &thread.string_table else {
            continue;
        };
        let Some(samples) = &thread.samples else {
            continue;
        };

        // Resolve frame name: funcTable.name -> stringTable, or frameTable.func -> stringTable.
        let resolve_name = |frame_idx: usize| -> String {
            if let Some(func_table) = &thread.func_table {
                let func_idx = frame_table.func.get(frame_idx).copied().unwrap_or(0);
                let name_idx = func_table.name.get(func_idx).copied().unwrap_or(0);
                string_table
                    .get(name_idx)
                    .cloned()
                    .unwrap_or_else(|| format!("frame-{frame_idx}"))
            } else {
                let func_idx = frame_table.func.get(frame_idx).copied().unwrap_or(0);
                string_table
                    .get(func_idx)
                    .cloned()
                    .unwrap_or_else(|| format!("frame-{frame_idx}"))
            }
        };

        /// Unwind a stack index into a list of frame indices (root first).
        fn unwind_stack(stack_table: &GeckoStackTable, stack_idx: usize) -> Vec<usize> {
            let mut result = Vec::new();
            let mut idx = Some(stack_idx);
            while let Some(i) = idx {
                if i >= stack_table.frame.len() {
                    break;
                }
                result.push(stack_table.frame[i]);
                idx = if i < stack_table.prefix.len() {
                    stack_table.prefix[i]
                } else {
                    None
                };
            }
            result.reverse();
            result
        }

        // Process samples to build frames.
        struct ActiveFrame {
            frame_idx: usize,
            frame_table_idx: usize,
        }

        let mut active_stacks: Vec<ActiveFrame> = Vec::new();

        for (i, stack_opt) in samples.stack.iter().enumerate() {
            let sample_time = samples.time.get(i).copied().unwrap_or(0.0) + profile_start;
            let next_time = samples
                .time
                .get(i + 1)
                .map(|t| t + profile_start)
                .unwrap_or(sample_time + interval);

            let stack = match stack_opt {
                Some(si) => unwind_stack(stack_table, *si),
                None => Vec::new(),
            };

            // Find common prefix.
            let mut common_len = 0;
            for (j, active) in active_stacks.iter().enumerate() {
                if j < stack.len() && stack[j] == active.frame_table_idx {
                    common_len = j + 1;
                } else {
                    break;
                }
            }

            // Close divergent frames.
            while active_stacks.len() > common_len {
                if let Some(af) = active_stacks.pop() {
                    all_frames[af.frame_idx].end = sample_time;
                }
            }

            // Open new frames.
            for (depth, &ft_idx) in stack.iter().enumerate().skip(common_len) {
                let name = resolve_name(ft_idx);
                let parent_id = if depth > 0 {
                    active_stacks.last().map(|af| all_frames[af.frame_idx].id)
                } else {
                    None
                };

                let id = next_id;
                next_id += 1;
                let fidx = all_frames.len();

                all_frames.push(Frame {
                    id,
                    name,
                    start: sample_time,
                    end: next_time,
                    depth: depth as u32,
                    category: thread.name.clone(),
                    parent: parent_id,
                    self_time: 0.0,
                    thread: None,
                });

                active_stacks.push(ActiveFrame {
                    frame_idx: fidx,
                    frame_table_idx: ft_idx,
                });
            }
        }

        // Close remaining frames.
        let last_time = samples.time.last().map(|t| t + profile_start + interval);
        if let Some(end_t) = last_time {
            for af in &active_stacks {
                all_frames[af.frame_idx].end = end_t;
            }
        }
    }

    // Compute self times.
    let child_time = {
        let mut map = std::collections::HashMap::<u64, f64>::new();
        for f in &all_frames {
            if let Some(pid) = f.parent {
                *map.entry(pid).or_default() += f.duration();
            }
        }
        map
    };
    for f in &mut all_frames {
        let children_total = child_time.get(&f.id).copied().unwrap_or(0.0);
        f.self_time = (f.duration() - children_total).max(0.0);
    }

    let start_time = all_frames
        .iter()
        .map(|f| f.start)
        .fold(f64::INFINITY, f64::min);
    let end_time = all_frames
        .iter()
        .map(|f| f.end)
        .fold(f64::NEG_INFINITY, f64::max);

    Ok(Profile::new(ProfileMetadata {
            name: gecko.meta.as_ref().and_then(|m| m.product.clone()),
            start_time: if start_time.is_finite() {
                start_time
            } else {
                0.0
            },
            end_time: if end_time.is_finite() { end_time } else { 0.0 },
            format: "firefox".to_string(),
            time_domain: None,
        },
        all_frames,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic_gecko_profile() {
        let json = r#"{
            "meta": {
                "interval": 1.0,
                "startTime": 0.0,
                "product": "Firefox"
            },
            "threads": [{
                "name": "GeckoMain",
                "stackTable": {
                    "frame": [0, 1],
                    "prefix": [null, 0]
                },
                "frameTable": {
                    "func": [0, 1],
                    "category": [null, null]
                },
                "funcTable": {
                    "name": [0, 1]
                },
                "stringTable": ["main", "work"],
                "samples": {
                    "stack": [1, 1, 0],
                    "time": [0.0, 1.0, 2.0]
                }
            }]
        }"#;

        let profile = parse_firefox(json.as_bytes()).unwrap();
        assert_eq!(profile.metadata.format, "firefox");
        assert_eq!(profile.metadata.name.as_deref(), Some("Firefox"));
        assert!(!profile.frames.is_empty());

        // Should have "main" and "work" frames
        let has_main = profile.frames.iter().any(|f| f.name == "main");
        let has_work = profile.frames.iter().any(|f| f.name == "work");
        assert!(has_main);
        assert!(has_work);
    }

    #[test]
    fn no_threads_errors() {
        let json = r#"{"threads":[]}"#;
        assert!(parse_firefox(json.as_bytes()).is_err());
    }
}
