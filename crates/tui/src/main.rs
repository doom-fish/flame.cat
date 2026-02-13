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

    renderer::render_tui(&profile)?;
    Ok(())
}
