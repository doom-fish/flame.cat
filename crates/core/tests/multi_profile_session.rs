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
    println!("Chrome time_domain: {:?}", chrome_profile.meta.time_domain);
    println!("React  time_domain: {:?}", react_profile.meta.time_domain);

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

    // React profile uses PerformanceNow, Chrome has navigationStart anchor.
    // Perfect alignment: React offset = navigationStart from Chrome trace.
    let chrome_nav_start = chrome_entry
        .profile
        .meta
        .time_domain
        .as_ref()
        .and_then(|td| td.navigation_start_us)
        .expect("Chrome trace should have navigationStart");
    assert!(
        (react_entry.offset_us - chrome_nav_start).abs() < 1.0,
        "React offset should equal Chrome navigationStart for perfect alignment: got {:.0}, expected {:.0}",
        react_entry.offset_us,
        chrome_nav_start,
    );

    // React commit at performance.now()=2836.4ms should map to:
    // session_time = navigationStart + 2836400µs
    let expected_react_start = chrome_nav_start + 2_836_400.0;
    assert!(
        (react_entry.session_start() - expected_react_start).abs() < 10.0,
        "React session start should be navigationStart + commit_timestamp: got {:.0}, expected {:.0}",
        react_entry.session_start(),
        expected_react_start,
    );

    // React profile should fall within Chrome trace time range
    assert!(
        react_entry.session_start() >= chrome_entry.session_start(),
        "React should start after Chrome trace start",
    );
    assert!(
        react_entry.session_end() <= chrome_entry.session_end(),
        "React should end before Chrome trace end",
    );

    // Manual offset adjustment should work
    let original_react_start = react_entry.session_start();
    session.profiles_mut()[1].offset_us += 1000.0; // shift React profile by 1ms
    let adjusted_react_start = session.profiles()[1].session_start();
    assert!(
        (adjusted_react_start - original_react_start - 1000.0).abs() < 1.0,
        "Manual offset should shift session time by 1000µs"
    );

    println!(
        "\n✅ Multi-profile session works: Chrome trace + React DevTools auto-aligned and offset-adjustable"
    );
}
