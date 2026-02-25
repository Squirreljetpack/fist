use crossterm::{
    cursor::MoveTo,
    event::{self, Event},
    execute,
    terminal::{Clear, ClearType, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::Constraint,
    style::{Color, Modifier, Style, Stylize},
    widgets::{Cell, Row, Table},
};
use std::io::{Result, stdout};

pub fn display_ratatui_styles() -> Result<()> {
    enable_raw_mode()?;

    let mut stdout = stdout();
    execute!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;

    let backend = CrosstermBackend::new(stdout);
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

    let modifier_tests = [
        Modifier::BOLD,
        Modifier::ITALIC,
        Modifier::UNDERLINED,
        Modifier::CROSSED_OUT,
        Modifier::DIM,
        Modifier::SLOW_BLINK,
        Modifier::RAPID_BLINK,
    ];

    let mut global_style = Style::new();

    loop {
        terminal.draw(|f| {
            let area = f.area();
            let col_width = 18u16;
            let columns = (area.width / col_width).max(1) as usize;

            let mut rows: Vec<Row> = Vec::new();

            // ---------- Colors ----------
            rows.push(
                Row::new(vec![Cell::from("Colors").style(
                    Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                )])
                .bottom_margin(1)
                .top_margin(1),
            );

            for chunk in colors.chunks(columns) {
                let cells: Vec<_> = chunk
                    .iter()
                    .map(|color| {
                        Cell::from(format!("{color:?}")).style(Style::default().fg(*color))
                    })
                    .collect();
                rows.push(Row::new(cells));
            }

            // ---------- Backgrounds ----------
            rows.push(
                Row::new(vec![Cell::from("Backgrounds").style(
                    Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                )])
                .bottom_margin(1)
                .top_margin(2),
            );

            for chunk in colors.chunks(columns) {
                let cells: Vec<_> = chunk
                    .iter()
                    .map(|color| {
                        Cell::from(format!("{color:?}"))
                            .style(Style::default().bg(*color).fg(Color::Black))
                    })
                    .collect();
                rows.push(Row::new(cells));
            }

            // ---------- Modifiers ----------
            rows.push(
                Row::new(vec![Cell::from("Modifiers").style(
                    Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                )])
                .bottom_margin(1)
                .top_margin(2),
            );

            for modifier in modifier_tests.iter() {
                rows.push(Row::new(vec![
                    Cell::from(format!("{:?}", modifier))
                        .style(Style::default().add_modifier(*modifier)),
                ]));
            }

            // ---------- Exit ----------
            rows.push(
                Row::new(vec![Cell::from("Press any key"), Cell::from("to exit")])
                    .italic()
                    .top_margin(2),
            );
            rows.push(Row::new(vec![Cell::from("Press d to"), Cell::from("toggle dim")]).italic());

            let widths = vec![Constraint::Length(col_width); columns.max(1)];
            let table = Table::new(rows, widths).style(global_style);
            f.render_widget(table, area);
        })?;

        // ---------- Input ----------
        if event::poll(std::time::Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    event::KeyCode::Char('d') => {
                        if global_style.has_modifier(Modifier::DIM) {
                            global_style = global_style.remove_modifier(Modifier::DIM);
                        } else {
                            global_style = global_style.add_modifier(Modifier::DIM);
                        }
                    }
                    _ => break,
                }
            }
        }
    }

    disable_raw_mode()?;
    Ok(())
}
