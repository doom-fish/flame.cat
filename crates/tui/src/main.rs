mod renderer;

use std::path::PathBuf;

use anyhow::Result;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: flame-cat <profile.json>");
        std::process::exit(1);
    }

    let path = PathBuf::from(&args[1]);
    let data = std::fs::read(&path)?;
    let profile = flame_cat_core::parsers::parse_auto_visual(&data)?;

    let viewport = flame_cat_protocol::Viewport {
        x: 0.0,
        y: 0.0,
        width: 120.0,
        height: 40.0,
        dpr: 1.0,
    };
    let commands = flame_cat_core::views::time_order::render_time_order(
        &profile,
        &viewport,
        profile.meta.start_time,
        profile.meta.end_time,
    );

    renderer::render_tui(&profile, &commands)?;
    Ok(())
}
