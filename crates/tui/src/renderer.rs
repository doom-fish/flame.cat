use std::io::stdout;

use anyhow::Result;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseEventKind,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use flame_cat_protocol::{RenderCommand, ThemeToken, VisualProfile};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders},
};

fn theme_to_color(token: &ThemeToken) -> Color {
    match token {
        ThemeToken::FlameHot => Color::Red,
        ThemeToken::FlameWarm => Color::Yellow,
        ThemeToken::FlameCold => Color::Blue,
        ThemeToken::FlameNeutral => Color::Gray,
        ThemeToken::LaneBackground => Color::Black,
        ThemeToken::LaneBorder => Color::DarkGray,
        ThemeToken::LaneHeaderBackground => Color::DarkGray,
        ThemeToken::LaneHeaderText => Color::White,
        ThemeToken::TextPrimary => Color::White,
        ThemeToken::TextSecondary => Color::Gray,
        ThemeToken::TextMuted => Color::DarkGray,
        ThemeToken::SelectionHighlight => Color::Green,
        ThemeToken::HoverHighlight => Color::LightYellow,
        ThemeToken::Background => Color::Black,
        ThemeToken::Surface => Color::Black,
        ThemeToken::Border => Color::DarkGray,
        ThemeToken::ToolbarBackground => Color::DarkGray,
        ThemeToken::ToolbarText => Color::White,
        ThemeToken::ToolbarTabActive => Color::Green,
        ThemeToken::ToolbarTabHover => Color::Gray,
        ThemeToken::MinimapBackground => Color::Black,
        ThemeToken::MinimapViewport => Color::DarkGray,
        ThemeToken::TableRowEven => Color::Black,
        ThemeToken::TableRowOdd => Color::Rgb(20, 20, 20),
        ThemeToken::TableHeaderBackground => Color::DarkGray,
        ThemeToken::TableBorder => Color::DarkGray,
        ThemeToken::BarFill => Color::Green,
        ThemeToken::SearchHighlight => Color::LightYellow,
        ThemeToken::CounterFill => Color::Rgb(60, 120, 200),
        ThemeToken::CounterLine => Color::Rgb(80, 160, 240),
        ThemeToken::CounterText => Color::Cyan,
        ThemeToken::MarkerLine => Color::Rgb(200, 100, 100),
        ThemeToken::MarkerText => Color::LightRed,
        ThemeToken::AsyncSpanFill => Color::Rgb(100, 150, 200),
        ThemeToken::AsyncSpanBorder => Color::Rgb(80, 120, 160),
        ThemeToken::FrameGood => Color::Green,
        ThemeToken::FrameWarning => Color::Yellow,
        ThemeToken::FrameDropped => Color::Red,
        ThemeToken::MinimapDensity => Color::Blue,
        ThemeToken::MinimapHandle => Color::LightBlue,
        ThemeToken::InlineLabelText => Color::White,
        ThemeToken::InlineLabelBackground => Color::Black,
        ThemeToken::FlowArrow => Color::DarkGray,
        ThemeToken::FlowArrowHead => Color::Gray,
    }
}

pub fn render_tui(profile: &VisualProfile) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut scroll_x: f64 = 0.0;
    let mut scroll_y: f64 = 0.0;
    let mut zoom: f64 = 1.0;

    let duration = profile.duration();

    loop {
        let term_size = terminal.size()?;
        let viewport = flame_cat_protocol::Viewport {
            x: 0.0,
            y: scroll_y,
            width: f64::from(term_size.width),
            height: f64::from(term_size.height.saturating_sub(2)),
            dpr: 1.0,
        };

        let visible_duration = duration / zoom;
        let view_start = profile.meta.start_time + scroll_x;
        let view_end = (view_start + visible_duration).min(profile.meta.end_time);

        let cmds = flame_cat_core::views::time_order::render_time_order(
            profile, &viewport, view_start, view_end, None,
        );

        terminal.draw(|frame| {
            let area = frame.area();

            // Header
            let header_area = Rect::new(0, 0, area.width, 1);
            let header = Block::default()
                .title(format!(
                    " flame.cat — {} spans | ←→ scroll | +/- zoom | q quit ",
                    profile.span_count()
                ))
                .style(Style::default().fg(Color::White).bg(Color::DarkGray));
            frame.render_widget(header, header_area);

            // Render commands as colored cells
            let content_area = Rect::new(0, 1, area.width, area.height.saturating_sub(1));
            let block = Block::default()
                .borders(Borders::NONE)
                .style(Style::default().bg(Color::Black));
            frame.render_widget(block, content_area);

            for cmd in &cmds {
                if let RenderCommand::DrawRect {
                    rect, color, label, ..
                } = cmd
                {
                    // Map floating-point coords to terminal cells
                    let col_scale = f64::from(content_area.width) / viewport.width;
                    let row_height = 20.0; // Each depth level maps to this many viewport units

                    let col = (rect.x * col_scale) as u16;
                    let row = (rect.y / row_height) as u16;
                    let width = ((rect.w * col_scale) as u16).max(1);

                    if row >= content_area.height || col >= content_area.width {
                        continue;
                    }

                    let fg = theme_to_color(color);
                    let label_str = label.as_deref().unwrap_or("");
                    let display: String = if (width as usize) >= label_str.len() + 2 {
                        format!(" {label_str:<w$}", w = (width as usize).saturating_sub(2))
                    } else {
                        "█".repeat(width as usize)
                    };

                    let clamped_width = width.min(content_area.width.saturating_sub(col));
                    let buf = frame.buffer_mut();
                    for (i, ch) in display.chars().take(clamped_width as usize).enumerate() {
                        let x = content_area.x + col + i as u16;
                        let y = content_area.y + row;
                        if x < content_area.x + content_area.width
                            && y < content_area.y + content_area.height
                        {
                            buf[(x, y)].set_char(ch).set_fg(fg).set_bg(Color::Black);
                        }
                    }
                }
            }
        })?;

        // Handle input
        if event::poll(std::time::Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Left => {
                        let step = (duration / zoom) * 0.1;
                        scroll_x = (scroll_x - step).max(0.0);
                    }
                    KeyCode::Right => {
                        let step = (duration / zoom) * 0.1;
                        scroll_x = (scroll_x + step).min(duration - duration / zoom).max(0.0);
                    }
                    KeyCode::Up => scroll_y = (scroll_y - 20.0).max(0.0),
                    KeyCode::Down => scroll_y += 20.0,
                    KeyCode::Char('+') | KeyCode::Char('=') => {
                        zoom *= 1.3;
                    }
                    KeyCode::Char('-') => {
                        zoom = (zoom / 1.3).max(1.0);
                    }
                    _ => {}
                },
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::ScrollDown => scroll_y += 20.0,
                    MouseEventKind::ScrollUp => scroll_y = (scroll_y - 20.0).max(0.0),
                    MouseEventKind::ScrollLeft => scroll_x = (scroll_x - 10.0).max(0.0),
                    MouseEventKind::ScrollRight => scroll_x += 10.0,
                    _ => {}
                },
                _ => {}
            }
        }
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
