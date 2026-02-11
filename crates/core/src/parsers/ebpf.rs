use thiserror::Error;

use crate::model::{Frame, Profile, ProfileMetadata};

#[derive(Debug, Error)]
pub enum EbpfParseError {
    #[error("invalid UTF-8: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("no stack data found")]
    Empty,
}

/// Parse eBPF profiler output into a `Profile`.
///
/// Supports common eBPF stack trace formats:
///
/// 1. **bpftrace/bcc format** — stack blocks separated by blank lines:
///    ```text
///    @[
///        func_c
///        func_b
///        func_a
///    ]: 42
///    ```
///
/// 2. **perf script with eBPF** — process header + indented stack:
///    ```text
///    process_name 1234 1234.567890:   1234 cycles:
///        ffffffff810a func_a+0x10
///        ffffffff810b func_b+0x20
///    ```
///
/// 3. **Collapsed stacks** (delegated to collapsed parser, detected at higher level).
///
/// This parser auto-detects between bpftrace block format and perf script format.
pub fn parse_ebpf(data: &[u8]) -> Result<Profile, EbpfParseError> {
    let text = std::str::from_utf8(data)?;

    // Detect format: bpftrace uses `@[` markers, perf script uses indented hex addresses.
    if text.contains("@[") {
        parse_bpftrace(text)
    } else {
        parse_perf_script(text)
    }
}

/// Parse bpftrace/bcc output format.
///
/// Format:
/// ```text
/// @[
///     frame_leaf
///     frame_mid
///     frame_root
/// ]: count
/// ```
fn parse_bpftrace(text: &str) -> Result<Profile, EbpfParseError> {
    let mut frames: Vec<Frame> = Vec::new();
    let mut next_id: u64 = 0;
    let mut offset: f64 = 0.0;

    // Parse blocks between @[ ... ]: count
    let mut i = 0;
    let chars: Vec<char> = text.chars().collect();

    while i < chars.len() {
        // Find @[
        if i + 1 < chars.len() && chars[i] == '@' && chars[i + 1] == '[' {
            i += 2;
            // Collect stack lines until ]:
            let mut stack_lines: Vec<String> = Vec::new();
            let mut block = String::new();

            while i < chars.len() {
                if chars[i] == ']' && i + 1 < chars.len() && chars[i + 1] == ':' {
                    break;
                }
                block.push(chars[i]);
                i += 1;
            }

            for line in block.lines() {
                let line = line.trim();
                if !line.is_empty() {
                    stack_lines.push(line.to_string());
                }
            }

            // Skip past ]:
            if i < chars.len() && chars[i] == ']' {
                i += 2; // skip ]: 
            }

            // Read count
            let mut count_str = String::new();
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == ' ') {
                if chars[i].is_ascii_digit() {
                    count_str.push(chars[i]);
                }
                i += 1;
            }
            let count: f64 = count_str.trim().parse().unwrap_or(1.0);

            // bpftrace stacks are leaf-first; reverse for root-first.
            stack_lines.reverse();

            let sample_end = offset + count;
            let mut parent_id: Option<u64> = None;

            for (depth, name) in stack_lines.iter().enumerate() {
                // Strip address prefix if present (e.g., "ffffffff810a func_name")
                let clean_name = strip_address(name);
                let is_leaf = depth == stack_lines.len() - 1;

                let id = next_id;
                next_id += 1;

                frames.push(Frame {
                    id,
                    name: clean_name,
                    start: offset,
                    end: sample_end,
                    depth: depth as u32,
                    category: Some("ebpf".to_string()),
                    parent: parent_id,
                    self_time: if is_leaf { count } else { 0.0 },
                    thread: None,
                });

                parent_id = Some(id);
            }

            offset = sample_end;
        } else {
            i += 1;
        }
    }

    if frames.is_empty() {
        return Err(EbpfParseError::Empty);
    }

    compute_self_times(&mut frames);
    build_profile(frames, "ebpf")
}

/// Parse `perf script` output format.
///
/// Format:
/// ```text
/// process_name pid timestamp: event:
///     addr func+offset (module)
///     addr func+offset (module)
///
/// ```
fn parse_perf_script(text: &str) -> Result<Profile, EbpfParseError> {
    let mut frames: Vec<Frame> = Vec::new();
    let mut next_id: u64 = 0;
    let mut offset: f64 = 0.0;

    let mut current_stack: Vec<String> = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            // End of a stack block — emit frames.
            if !current_stack.is_empty() {
                // perf script stacks are leaf-first; reverse.
                current_stack.reverse();
                let sample_end = offset + 1.0;
                let mut parent_id: Option<u64> = None;

                for (depth, name) in current_stack.iter().enumerate() {
                    let is_leaf = depth == current_stack.len() - 1;
                    let id = next_id;
                    next_id += 1;

                    frames.push(Frame {
                        id,
                        name: name.clone(),
                        start: offset,
                        end: sample_end,
                        depth: depth as u32,
                        category: Some("perf".to_string()),
                        parent: parent_id,
                        self_time: if is_leaf { 1.0 } else { 0.0 },
                        thread: None,
                    });

                    parent_id = Some(id);
                }

                offset = sample_end;
                current_stack.clear();
            }
            continue;
        }

        // Stack frame line: starts with whitespace + hex address
        if line.starts_with('\t') || line.starts_with("        ") || line.starts_with("    ") {
            let name = parse_perf_frame(trimmed);
            if !name.is_empty() {
                current_stack.push(name);
            }
        }
        // Otherwise it's a header line (process name, pid, etc.) — skip.
    }

    // Flush last stack.
    if !current_stack.is_empty() {
        current_stack.reverse();
        let sample_end = offset + 1.0;
        let mut parent_id: Option<u64> = None;

        for (depth, name) in current_stack.iter().enumerate() {
            let is_leaf = depth == current_stack.len() - 1;
            let id = next_id;
            next_id += 1;

            frames.push(Frame {
                id,
                name: name.clone(),
                start: offset,
                end: sample_end,
                depth: depth as u32,
                category: Some("perf".to_string()),
                parent: parent_id,
                self_time: if is_leaf { 1.0 } else { 0.0 },
                thread: None,
            });

            parent_id = Some(id);
        }
    }

    if frames.is_empty() {
        return Err(EbpfParseError::Empty);
    }

    compute_self_times(&mut frames);
    build_profile(frames, "ebpf-perf")
}

/// Parse a perf script frame line like `ffffffff810a func_name+0x10 (/path/module)`
fn parse_perf_frame(line: &str) -> String {
    let parts: Vec<&str> = line.splitn(2, ' ').collect();
    if parts.len() < 2 {
        return strip_address(line);
    }

    // Second part is "func_name+0x10 (/path/module)" — take function name
    let func_part = parts[1].trim();

    // Remove module in parens at the end
    let func_part = if let Some(paren_pos) = func_part.rfind('(') {
        func_part[..paren_pos].trim()
    } else {
        func_part
    };

    // Remove +0xOFFSET suffix
    if let Some(plus_pos) = func_part.rfind('+') {
        func_part[..plus_pos].to_string()
    } else {
        func_part.to_string()
    }
}

/// Strip leading hex address from a frame name.
fn strip_address(name: &str) -> String {
    let trimmed = name.trim();
    // Check if starts with hex address (all hex chars before a space)
    if let Some(space_pos) = trimmed.find(' ') {
        let prefix = &trimmed[..space_pos];
        if prefix.chars().all(|c| c.is_ascii_hexdigit()) && prefix.len() >= 4 {
            return trimmed[space_pos + 1..].trim().to_string();
        }
    }
    trimmed.to_string()
}

fn compute_self_times(frames: &mut [Frame]) {
    let child_time = {
        let mut map = std::collections::HashMap::<u64, f64>::new();
        for f in frames.iter() {
            if let Some(pid) = f.parent {
                *map.entry(pid).or_default() += f.duration();
            }
        }
        map
    };
    for f in frames.iter_mut() {
        let children_total = child_time.get(&f.id).copied().unwrap_or(0.0);
        f.self_time = (f.duration() - children_total).max(0.0);
    }
}

fn build_profile(frames: Vec<Frame>, format: &str) -> Result<Profile, EbpfParseError> {
    let start_time = frames.iter().map(|f| f.start).fold(f64::INFINITY, f64::min);
    let end_time = frames
        .iter()
        .map(|f| f.end)
        .fold(f64::NEG_INFINITY, f64::max);

    Ok(Profile {
        metadata: ProfileMetadata {
            name: None,
            start_time: if start_time.is_finite() {
                start_time
            } else {
                0.0
            },
            end_time: if end_time.is_finite() { end_time } else { 0.0 },
            format: format.to_string(),
            time_domain: None,
        },
        frames,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_bpftrace_format() {
        let input = b"@[\n    leaf_func\n    mid_func\n    root_func\n]: 42\n";
        let profile = parse_ebpf(input).unwrap();
        assert!(profile.metadata.format.starts_with("ebpf"));
        assert_eq!(profile.frames.len(), 3);

        // Should be root-first after reversal
        assert_eq!(profile.frames[0].name, "root_func");
        assert_eq!(profile.frames[0].depth, 0);
        assert_eq!(profile.frames[2].name, "leaf_func");
        assert_eq!(profile.frames[2].depth, 2);
    }

    #[test]
    fn parse_perf_script_format() {
        let input = b"process 1234 12345.678: 1 cycles:\n\tffffffff810a func_a+0x10 (/lib/mod)\n\tffffffff810b func_b+0x20 (/lib/mod)\n\n";
        let profile = parse_ebpf(input).unwrap();
        assert_eq!(profile.metadata.format, "ebpf-perf");
        assert_eq!(profile.frames.len(), 2);

        // perf stacks are leaf-first, reversed to root-first
        assert_eq!(profile.frames[0].name, "func_b");
        assert_eq!(profile.frames[1].name, "func_a");
    }

    #[test]
    fn empty_input_errors() {
        assert!(parse_ebpf(b"").is_err());
    }

    #[test]
    fn strip_hex_address() {
        assert_eq!(strip_address("ffffffff810a func_name"), "func_name");
        assert_eq!(strip_address("regular_name"), "regular_name");
    }
}
