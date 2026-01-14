use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
};
use std::io::stdout;

pub fn display_ratatui_colors() -> std::io::Result<()> {
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    let colors = [
        Color::Black,
        Color::Red,
        Color::Green,
        Color::Yellow,
        Color::Blue,
        Color::Magenta,
        Color::Cyan,
        Color::Gray,
        Color::DarkGray,
        Color::LightRed,
        Color::LightGreen,
        Color::LightYellow,
        Color::LightBlue,
        Color::LightMagenta,
        Color::LightCyan,
        Color::White,
    ];

    terminal.draw(|f| {
        let chunks = Layout::default()
            .constraints(
                colors
                    .iter()
                    .map(|_| Constraint::Length(5))
                    .collect::<Vec<_>>(),
            )
            .split(f.area());

        for (i, color) in colors.iter().enumerate() {
            let p = Paragraph::new(format!("{:?}", color))
                .style(Style::default().fg(*color))
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(p, chunks[i]);
        }
    })?;

    Ok(())
}
