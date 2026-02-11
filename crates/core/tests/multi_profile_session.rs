//! Integration test: load a Chrome trace and a React DevTools profile into a
//! multi-profile Session and verify alignment, lane creation, and metadata.

use flame_cat_core::model::Session;
use flame_cat_core::parsers::parse_auto_visual;

#[test]
fn load_chrome_and_react_into_session() {
    let chrome_data = include_bytes!("fixtures/chrome-trace-sample.json");
    let react_data = include_bytes!("fixtures/react-devtools-metronome.json");

    // Parse both profiles
    let chrome_profile = parse_auto_visual(chrome_data).expect("failed to parse Chrome trace");
    let react_profile =
        parse_auto_visual(react_data).expect("failed to parse React DevTools export");

    // Chrome profile should have spans and metadata
    assert!(
        !chrome_profile.threads.is_empty(),
        "Chrome profile should have threads"
    );
    let chrome_span_count: usize = chrome_profile.threads.iter().map(|t| t.spans.len()).sum();
    assert!(chrome_span_count > 0, "Chrome profile should have spans");
    println!(
        "Chrome: {} threads, {} spans, {:.0}µs – {:.0}µs ({:?})",
        chrome_profile.threads.len(),
        chrome_span_count,
        chrome_profile.meta.start_time,
        chrome_profile.meta.end_time,
        chrome_profile.meta.value_unit,
    );

    // React profile should have spans
    assert!(
        !react_profile.threads.is_empty(),
        "React profile should have threads"
    );
    let react_span_count: usize = react_profile.threads.iter().map(|t| t.spans.len()).sum();
    assert!(react_span_count > 0, "React profile should have spans");
    println!(
        "React:  {} threads, {} spans, {:.0}µs – {:.0}µs ({:?})",
        react_profile.threads.len(),
        react_span_count,
        react_profile.meta.start_time,
        react_profile.meta.end_time,
        react_profile.meta.value_unit,
    );

    // Log time domain info
    println!(
        "Chrome time_domain: {:?}",
        chrome_profile.meta.time_domain
    );
    println!(
        "React  time_domain: {:?}",
        react_profile.meta.time_domain
    );

    // Create session with Chrome trace first
    let mut session = Session::from_profile(chrome_profile, "chrome-trace.json");
    assert_eq!(session.len(), 1);

    // Add React profile
    session.add_profile(react_profile, "react-devtools.json");
    assert_eq!(session.len(), 2);

    // Session should span both profiles
    let start = session.start_time();
    let end = session.end_time();
    let duration = session.duration();
    println!("Session: {start:.0}µs – {end:.0}µs (duration: {duration:.0}µs)");
    assert!(duration > 0.0, "Session duration should be positive");
    assert!(start.is_finite(), "Session start should be finite");
    assert!(end.is_finite(), "Session end should be finite");

    // Both profiles should be accessible
    let entries = session.profiles();
    assert_eq!(entries.len(), 2);

    let chrome_entry = &entries[0];
    let react_entry = &entries[1];

    println!(
        "Chrome offset: {:.0}µs, session range: {:.0} – {:.0}",
        chrome_entry.offset_us,
        chrome_entry.session_start(),
        chrome_entry.session_end(),
    );
    println!(
        "React  offset: {:.0}µs, session range: {:.0} – {:.0}",
        react_entry.offset_us,
        react_entry.session_start(),
        react_entry.session_end(),
    );

    // React profile (no time domain) should be auto-aligned to Chrome's start
    assert!(
        react_entry.offset_us.abs() > 1.0,
        "React offset should be non-zero (auto-aligned): got {}",
        react_entry.offset_us,
    );
    assert!(
        (react_entry.session_start() - chrome_entry.session_start()).abs() < 1.0,
        "React session start should match Chrome start after auto-alignment: React={:.0}, Chrome={:.0}",
        react_entry.session_start(),
        chrome_entry.session_start(),
    );

    // Manual offset adjustment should work
    let original_react_start = react_entry.session_start();
    session.profiles_mut()[1].offset_us += 1000.0; // shift React profile by 1ms
    let adjusted_react_start = session.profiles()[1].session_start();
    assert!(
        (adjusted_react_start - original_react_start - 1000.0).abs() < 1.0,
        "Manual offset should shift session time by 1000µs"
    );

    println!("\n✅ Multi-profile session works: Chrome trace + React DevTools auto-aligned and offset-adjustable");
}
