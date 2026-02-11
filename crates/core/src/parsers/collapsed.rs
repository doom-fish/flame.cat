use thiserror::Error;

use crate::model::{Frame, Profile, ProfileMetadata};

#[derive(Debug, Error)]
pub enum CollapsedParseError {
    #[error("invalid UTF-8: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("no valid stack lines found")]
    Empty,
}

/// Parse Brendan Gregg's collapsed/folded stack format.
///
/// Each line has the format: `stack_frame;stack_frame;... count`
/// where frames are separated by `;` and the count is the last whitespace-separated token.
///
/// Used by: `perf script | stackcollapse-perf.pl`, dtrace, FlameGraph tools.
pub fn parse_collapsed(data: &[u8]) -> Result<Profile, CollapsedParseError> {
    let text = std::str::from_utf8(data)?;
    let mut frames: Vec<Frame> = Vec::new();
    let mut next_id: u64 = 0;
    let mut offset: f64 = 0.0;

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Split into stack and count: "a;b;c 42"
        let (stack_str, count) = match line.rfind(' ') {
            Some(pos) => {
                let count_str = line[pos + 1..].trim();
                let count: f64 = count_str.parse().unwrap_or(1.0);
                (line[..pos].trim(), count)
            }
            None => continue,
        };

        if stack_str.is_empty() {
            continue;
        }

        let stack_parts: Vec<&str> = stack_str.split(';').collect();

        // Create frames for this stack sample.
        // Each sample becomes a set of nested frames with duration = count.
        let sample_start = offset;
        let sample_end = offset + count;

        let mut parent_id: Option<u64> = None;
        for (depth, name) in stack_parts.iter().enumerate() {
            let name = name.trim();
            if name.is_empty() {
                continue;
            }
            let id = next_id;
            next_id += 1;

            let is_leaf = depth == stack_parts.len() - 1;

            frames.push(Frame {
                id,
                name: name.to_string(),
                start: sample_start,
                end: sample_end,
                depth: depth as u32,
                category: None,
                parent: parent_id,
                self_time: if is_leaf { count } else { 0.0 },
                thread: None,
            });

            parent_id = Some(id);
        }

        offset = sample_end;
    }

    if frames.is_empty() {
        return Err(CollapsedParseError::Empty);
    }

    // Recompute self_time: each non-leaf frame's self_time = duration - sum(children)
    let child_time = {
        let mut map = std::collections::HashMap::<u64, f64>::new();
        for f in &frames {
            if let Some(pid) = f.parent {
                *map.entry(pid).or_default() += f.duration();
            }
        }
        map
    };
    for f in &mut frames {
        let children_total = child_time.get(&f.id).copied().unwrap_or(0.0);
        f.self_time = (f.duration() - children_total).max(0.0);
    }

    let start_time = frames.iter().map(|f| f.start).fold(f64::INFINITY, f64::min);
    let end_time = frames
        .iter()
        .map(|f| f.end)
        .fold(f64::NEG_INFINITY, f64::max);

    Ok(Profile::new(ProfileMetadata {
            name: None,
            start_time: if start_time.is_finite() {
                start_time
            } else {
                0.0
            },
            end_time: if end_time.is_finite() { end_time } else { 0.0 },
            format: "collapsed".to_string(),
            time_domain: None,
        },
        frames,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic_collapsed() {
        let input = b"main;foo;bar 10\nmain;foo;baz 20\nmain;qux 5\n";
        let profile = parse_collapsed(input).unwrap();
        assert_eq!(profile.metadata.format, "collapsed");

        // 3 lines: main;foo;bar(3 frames) + main;foo;baz(3) + main;qux(2) = 8 frames
        assert_eq!(profile.frames.len(), 8);
        assert_eq!(profile.metadata.end_time, 35.0);

        // Leaf "bar" should have self_time = 10
        let bar = profile.frames.iter().find(|f| f.name == "bar").unwrap();
        assert_eq!(bar.self_time, 10.0);
        assert_eq!(bar.depth, 2);
    }

    #[test]
    fn skips_comments_and_empty_lines() {
        let input = b"# comment\n\nmain;foo 5\n";
        let profile = parse_collapsed(input).unwrap();
        assert_eq!(profile.frames.len(), 2);
    }

    #[test]
    fn empty_input_errors() {
        let result = parse_collapsed(b"");
        assert!(result.is_err());
    }
}
