use matchmaker::nucleo::{Line, Span, Style};
use ratatui::{
    style::{Color, Modifier},
    text::Text,
};

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
pub fn bold_segments<'a, I, J>(segments: I, indices: J, normal_style: Style) -> Vec<Span<'a>>
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
    no_column: bool,
) -> Option<(String, String, Text)> {
    let mut state: usize = 0;
    let mut path = String::new();
    let mut loc = String::new();
    let mut content_spans: Vec<Span> = Vec::new();

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
                    if c == '\0' {
                        state = 1;
                    } else {
                        path.push(c);
                    }
                }
                1 => {
                    if c == match_sep || c == ctx_sep {
                        loc.push(c);
                        // If it's context, or we don't expect a column, we're done with the prefix
                        if c == ctx_sep || no_column {
                            state = 3;
                        } else {
                            state = 2;
                        }
                    } else if bytes[current_pos].is_ascii_digit() {
                        loc.push(c);
                    } else {
                        return None;
                    }
                }
                2 => {
                    if c == match_sep {
                        loc.push(c);
                        state = 3;
                    } else if bytes[current_pos].is_ascii_digit() {
                        loc.push(c);
                    } else {
                        return None; // always expect column after row index
                    }
                }
                _ => unreachable!(),
            }

            if state == 3 {
                let remaining = &content[current_pos + 1..];
                if !remaining.is_empty() {
                    content_spans.push(Span::styled(remaining.to_string(), span.style));
                }
                break;
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

pub fn extract_rg_line_no_path(line: &Line, out: &mut String, no_column: bool) -> bool {
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

                        if no_column {
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
                            state = State::AfterFirstColon;
                        }
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

pub fn is_empty(text: &Text<'_>) -> bool {
    text.lines.iter().all(|l| l.spans.is_empty())
}

/// Parse a `line:col` (or bare `line`) location string into `(line, col)`.
/// Returns `None` if the line component is not a number. `col` defaults to 0.
pub fn parse_loc(s: &str) -> Option<(u32, u32)> {
    let mut parts = s.splitn(2, ':');
    let line = parts.next()?.trim().parse::<u32>().ok()?;
    let col = parts
        .next()
        .and_then(|c| c.split(':').next()?.trim().parse::<u32>().ok())
        .unwrap_or(0);
    Some((line, col))
}

/// Parse the first `line:col:` pair out of a `"line:col:"`-joined string
/// (as built by [`extract_rg_line_no_path`]). `col` is 0 when `no_column` was
/// in effect (entries are then `"line:"`).
pub fn first_loc(s: &str) -> Option<(u32, u32)> {
    // entries are ':'-terminated pairs; take up through the second ':' (or end)
    let mut idx = 0;
    let mut colons = 0;
    for (i, ch) in s.char_indices() {
        if ch == ':' {
            colons += 1;
            if colons == 2 {
                idx = i;
                break;
            }
        }
    }
    let first = if idx > 0 {
        &s[..idx]
    } else {
        s.trim_end_matches(':')
    };
    parse_loc(first)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ansi_to_tui::IntoText;

    #[test]
    fn test_parse_rg_line_ansi() {
        let raw = "\x1b[0m\x1b[35msrc/run/previewer.rs\x1b[0m\0\x1b[0m\x1b[32m18\x1b[0m:\x1b[0m32\x1b[0m:pub fn make_previewer(";
        let mut text = raw.as_bytes().into_text().unwrap();
        let parsed = parse_rg_line(text.lines.remove(0), ':', '-', false);
        eprintln!("parsed: {:?}", parsed);
        assert!(parsed.is_some());
        let (path, loc, content) = parsed.unwrap();
        assert_eq!(path, "src/run/previewer.rs");
        assert_eq!(loc, "18:32:");
        assert_eq!(content.to_string(), "pub fn make_previewer(");
    }

    #[test]
    fn test_extract_rg_line_no_path_multiline() {
        let raw_ctx = "\x1b[0m\x1b[32m27\x1b[0m-\x1b[0mlet queue = QUEUE;";
        let raw_line1 = "\x1b[0m\x1b[32m28\x1b[0m:\x1b[0m20\x1b[0m:previewer::make_previewer,";
        let raw_line2 =
            "\x1b[0m\x1b[32m253\x1b[0m:\x1b[0m21\x1b[0m:let previewer = make_previewer(";
        let tc = raw_ctx.as_bytes().into_text().unwrap();
        let t1 = raw_line1.as_bytes().into_text().unwrap();
        let t2 = raw_line2.as_bytes().into_text().unwrap();

        let mut places = String::new();
        let okc = extract_rg_line_no_path(&tc.lines[0], &mut places, false);
        let ok1 = extract_rg_line_no_path(&t1.lines[0], &mut places, false);
        let ok2 = extract_rg_line_no_path(&t2.lines[0], &mut places, false);
        assert!(!okc);
        assert!(ok1);
        assert!(ok2);
        eprintln!("places: {:?}", places);
        let loc = first_loc(&places);
        eprintln!("loc: {:?}", loc);
        assert_eq!(loc, Some((28, 20)));
    }
}
