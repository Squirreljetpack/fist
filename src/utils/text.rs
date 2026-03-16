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
    normal_style: Style,
) -> Vec<Span<'_>> {
    let mut spans = Vec::new();
    let mut indices_iter = indices.into_iter();
    let mut next_bold = indices_iter.next().unwrap_or(usize::MAX); // first index to bold

    let mut buffer = String::new();

    for (char_idx, c) in s.chars().enumerate() {
        if char_idx == next_bold {
            if !buffer.is_empty() {
                spans.push(Span::styled(std::mem::take(&mut buffer), normal_style));
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
        spans.push(Span::styled(buffer, normal_style));
    }

    spans
}
pub fn bold_segments<'a, I, J>(
    segments: I,
    indices: J,
    normal_style: Style,
) -> Vec<Span<'a>>
where
    I: IntoIterator<Item = &'a str>,
    J: IntoIterator<Item = usize>,
{
    let mut spans = Vec::new();
    let mut indices = indices.into_iter().peekable();
    let mut offset = 0;

    for s in segments {
        let len = s.chars().count();

        // collect indices that fall within this segment
        let mut local = Vec::new();
        while let Some(&idx) = indices.peek() {
            if idx < offset + len {
                local.push(idx - offset);
                indices.next();
            } else {
                break;
            }
        }

        spans.extend(bold_indices(s, local, normal_style));
        offset += len;
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

pub fn spans_to_owned(spans: Vec<Span<'_>>) -> Vec<Span<'static>> {
    spans
        .into_iter()
        .map(|span| Span {
            content: span.content.to_string().into(),
            style: span.style,
        })
        .collect()
}

pub fn parse_rg_line(
    line: Line,
    match_sep: char,
    ctx_sep: char,
) -> Option<(String, String, Text)> {
    let mut state: usize = 0;
    let mut path = String::new();
    let mut loc = String::new();
    let mut content_spans: Vec<Span> = Vec::new();
    let mut sep = '\0';

    for span in line.spans {
        let content = span.content.as_ref();

        if state == 3 {
            content_spans.push(span);
            continue;
        }

        let bytes = content.as_bytes();
        let mut current_pos = 0;

        while current_pos < bytes.len() {
            let c = bytes[current_pos] as char;

            match state {
                0 => {
                    if c == match_sep || c == ctx_sep {
                        sep = c;
                        state = 1;
                    } else {
                        path.push(c);
                    }
                }
                1 => {
                    if c == sep {
                        loc.push(c);
                        if sep == ctx_sep {
                            state = 3;
                            let remaining = &content[current_pos + 1..];
                            if !remaining.is_empty() {
                                content_spans.push(Span::styled(remaining.to_string(), span.style));
                            }
                            break;
                        } else {
                            state = 2;
                        }
                    } else if bytes[current_pos].is_ascii_digit() {
                        loc.push(c);
                    } else {
                        path.push(sep);
                        path.push_str(&loc);
                        path.push(c);
                        loc.clear();
                        state = 0;
                    }
                }
                2 => {
                    if c == sep {
                        loc.push(c);
                        state = 3;
                        let remaining = &content[current_pos + 1..];
                        if !remaining.is_empty() {
                            content_spans.push(Span::styled(remaining.to_string(), span.style));
                        }
                        break;
                    } else if bytes[current_pos].is_ascii_digit() {
                        loc.push(c);
                    } else {
                        path.push(sep);
                        path.push_str(&loc);
                        path.push(c);
                        loc.clear();
                        state = 0;
                    }
                }
                _ => unreachable!(),
            }

            current_pos += 1;
        }
    }

    if state == 3 {
        Some((path, loc, Text::from(Line::from(content_spans))))
    } else {
        None
    }
}

pub fn extract_rg_line_no_path(
    line: &Line,
    out: &mut String,
) -> bool {
    #[derive(Clone, Copy)]
    enum State {
        FirstDigits,
        AfterFirstColon,
        SecondDigits,
    }

    let mut state = State::FirstDigits;
    let mut len = 0usize;

    for span in &line.spans {
        for ch in span.content.chars() {
            match state {
                State::FirstDigits => {
                    if ch.is_ascii_digit() {
                        len += ch.len_utf8();
                    } else if ch == ':' && len > 0 {
                        len += 1;
                        state = State::AfterFirstColon;
                    } else {
                        return false;
                    }
                }
                State::AfterFirstColon => {
                    if ch.is_ascii_digit() {
                        len += ch.len_utf8();
                        state = State::SecondDigits;
                    } else {
                        return false;
                    }
                }
                State::SecondDigits => {
                    if ch.is_ascii_digit() {
                        len += ch.len_utf8();
                    } else if ch == ':' {
                        len += 1;

                        // success: push exactly the matched prefix
                        let mut remaining = len;
                        for span in &line.spans {
                            if remaining == 0 {
                                break;
                            }
                            let s = span.content.as_ref();
                            let take = remaining.min(s.len());
                            out.push_str(&s[..take]);
                            remaining -= take;
                        }

                        return true;
                    } else {
                        return false;
                    }
                }
            }
        }
    }

    false
}
