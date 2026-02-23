use matchmaker::nucleo::{Line, Span, Style};
use ratatui::{
    style::{Color, Modifier},
    text::Text,
};

// strum::IntoStaticStr,
#[derive(Copy, Clone, Debug, Default, strum::Display)]
pub enum ToastStyle {
    #[default]
    #[strum(serialize = "Note")]
    Normal,
    Info,
    Success,
    Warning,
    Error,
}

impl From<ToastStyle> for Style {
    fn from(val: ToastStyle) -> Self {
        match val {
            ToastStyle::Normal => Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
            ToastStyle::Info => Style::default().fg(Color::Blue),
            ToastStyle::Success => Style::default().fg(Color::Green),
            ToastStyle::Warning => Style::default().fg(Color::Yellow),
            ToastStyle::Error => Style::default().fg(Color::Red),
        }
    }
}

#[derive(Debug)]
pub enum ToastContent {
    List(Vec<Span<'static>>),
    Pair(Span<'static>, Span<'static>),
    Line(Line<'static>),
}

pub fn make_toast(toasts: &[(Span<'static>, ToastContent)]) -> Text<'static> {
    let lines = toasts.iter().map(|(prefix, content)| {
        let mut spans = Vec::new();
        spans.push(prefix.clone());

        match content {
            ToastContent::List(items) => {
                for (i, item) in items.iter().cloned().enumerate() {
                    if i > 0 {
                        spans.push(Span::raw(", "));
                    }
                    spans.push(item);
                }
            }
            ToastContent::Pair(a, b) => {
                spans.push(a.clone());
                spans.push(" → ".into());
                spans.push(b.clone());
            }
            ToastContent::Line(line) => {
                spans.extend(line.clone());
            }
        }

        Line::from(spans)
    });

    Text::from(lines.collect::<Vec<_>>())
}

pub fn bold_indices(
    s: &str,
    indices: impl IntoIterator<Item = usize>,
) -> Vec<Span<'_>> {
    let mut spans = Vec::new();
    let mut indices_iter = indices.into_iter();
    let mut next_bold = indices_iter.next().unwrap_or(usize::MAX); // first index to bold

    let mut buffer = String::new();

    for (char_idx, c) in s.chars().enumerate() {
        if char_idx == next_bold {
            if !buffer.is_empty() {
                spans.push(Span::raw(std::mem::take(&mut buffer)));
            }
            spans.push(Span::styled(
                c.to_string(),
                Style::default().add_modifier(Modifier::BOLD),
            ));
            next_bold = indices_iter.next().unwrap_or(usize::MAX);
        } else {
            buffer.push(c);
        }
    }

    if !buffer.is_empty() {
        spans.push(Span::raw(buffer));
    }

    spans
}

/// Convert `Text` into lines of plain `String`s
pub fn text_to_lines(text: &Text) -> Vec<String> {
    text.iter()
        .map(|spans| {
            spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect()
}

/// Convert `Text` into a single `String` with newlines
pub fn text_to_string(text: &Text) -> String {
    text_to_lines(text).join("\n")
}

/// Cleans a Text object by removing explicit 'Reset' colors and 'Not' modifiers.
/// This allows the Text to properly inherit styles from its parent container.
pub fn scrub_text_styles(text: &mut Text<'_>) {
    for line in &mut text.lines {
        for span in &mut line.spans {
            // 1. Handle Colors: If it's explicitly Reset, make it None (transparent/inherit)
            if span.style.fg == Some(Color::Reset) {
                span.style.fg = None;
            }
            if span.style.bg == Some(Color::Reset) {
                span.style.bg = None;
            }
            if span.style.underline_color == Some(Color::Reset) {
                span.style.underline_color = None;
            }

            span.style.sub_modifier = Modifier::default();
        }
    }
}
