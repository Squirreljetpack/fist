use std::path::{MAIN_SEPARATOR, Path, PathBuf};

use matchmaker::nucleo::{Line, Span, Style};
use ratatui::{
    style::{Color, Modifier},
    text::Text,
};

use crate::{abspath::AbsPath, cli::paths::home_dir};

#[derive(Copy, Clone, Debug, Default, strum::IntoStaticStr)]
pub enum ToastStyle {
    #[default]
    Normal,
    Info,
    Success,
    Warning,
    Error,
}

impl ToastStyle {
    pub fn to_style(self) -> Style {
        match self {
            ToastStyle::Normal => Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
            ToastStyle::Info => Style::default().fg(Color::Blue),
            ToastStyle::Success => Style::default().fg(Color::Green),
            ToastStyle::Warning => Style::default().fg(Color::Yellow),
            ToastStyle::Error => Style::default()
                .fg(Color::Red)
                .add_modifier(ratatui::style::Modifier::BOLD),
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

fn split_path_ends(
    path: &Path,
    start_count: usize,
    end_count: usize,
) -> (String, String) {
    let comps: Vec<_> = path.components().collect();

    let len = comps.len();

    if start_count + end_count >= len {
        return (path.to_string_lossy().into_owned(), String::new());
    }

    let first = comps[..start_count]
        .iter()
        .collect::<PathBuf>()
        .to_string_lossy()
        .into_owned();

    let last = comps[len - end_count..]
        .iter()
        .collect::<PathBuf>()
        .to_string_lossy()
        .into_owned();

    (first, last)
}

pub fn format_cwd_prompt(
    template: &str,
    cwd: &Path,
) -> String {
    let mut out = String::with_capacity(template.len());
    let mut chars = template.chars().peekable();
    // collapse home
    let cwd = if let Ok(stripped) = cwd.strip_prefix(home_dir()) {
        &PathBuf::from("~").join(stripped)
    } else {
        cwd
    };

    while let Some(ch) = chars.next() {
        if ch != '{' {
            out.push(ch);
            continue;
        }

        let mut spec = String::new();
        for c in chars.by_ref() {
            if c == '}' {
                break;
            }
            spec.push(c);
        }

        match spec.as_str() {
            "" => {
                out.push_str(&cwd.to_string_lossy());
            }
            _ => {
                match spec.split_once(':').map(|(x, y)| {
                    (
                        x.is_empty()
                            .then_some(0)
                            .or_else(|| x.parse::<usize>().ok()),
                        y.is_empty()
                            .then_some(0)
                            .or_else(|| y.parse::<usize>().ok()),
                    )
                }) {
                    Some((Some(s), Some(e))) => {
                        let (first, last) = split_path_ends(cwd, s, e);
                        if last.is_empty() {
                            // first is full path
                            out.push_str(&first);
                        } else {
                            out.push_str(&first);
                            out.push('…');
                            out.push(MAIN_SEPARATOR);
                            out.push_str(&last);
                        }
                    }
                    _ => {
                        out.push_str(&spec);
                    }
                }
            }
        }
    }

    out
}

use unicode_segmentation::UnicodeSegmentation;

pub fn grapheme_index_to_byte_index(
    s: &str,
    grapheme_index: u16,
) -> usize {
    s.grapheme_indices(true)
        .nth(grapheme_index as usize)
        .map_or(s.len(), |(i, _)| i)
}

pub fn bold_indices(
    s: &str,
    indices: impl IntoIterator<Item = usize>,
) -> Vec<Span<'_>> {
    let mut spans = Vec::new();
    let mut indices_iter = indices.into_iter();
    let mut next_bold = indices_iter.next().unwrap_or(usize::MAX); // first index to bold
    let char_idx = 0;

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

/// Split on whitespace, maintain within ', \ escape
pub fn split_shell_like(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = s.chars().peekable();

    let mut in_single = false;

    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                in_single = !in_single;
            }
            '\\' => {
                if let Some(next) = chars.next() {
                    cur.push(next);
                }
            }
            c if c.is_whitespace() && !in_single => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            _ => cur.push(c),
        }
    }

    if !cur.is_empty() {
        out.push(cur);
    }

    out
}

// replace {}, {s:e}, leave everything else intact. supports \ escape.


pub fn path_formatter(
    template: &str,
    path: &AbsPath,
) -> String {
    let mut out = String::with_capacity(template.len());
    let mut chars = template.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            // escape sequence
            if let Some(next) = chars.next() {
                out.push(next);
            }
            continue;
        }

        if ch != '{' {
            out.push(ch);
            continue;
        }

        // parse spec inside {}
        let mut spec = String::new();
        while let Some(&c) = chars.peek() {
            if c == '}' {
                chars.next(); // consume
                break;
            }
            spec.push(c);
            chars.next();
        }

        if spec.is_empty() {
            // {}
            out.push('\'');
            out.push_str(&path.to_string_lossy());
            out.push('\'');
        } else if spec.contains('.') {
            // contains dot → leave path unquoted
            out.push_str(&path.to_string_lossy());
        } else if let Some((a, b)) = spec.split_once(':') {
            // check if both a and b are integers (can be negative)
            let a_valid = a.is_empty() || a.parse::<i32>().is_ok();
            let b_valid = b.is_empty() || b.parse::<i32>().is_ok();
            if a_valid && b_valid {
                out.push('\'');
                out.push_str(&path.to_string_lossy());
                out.push('\'');
            } else {
                // invalid spec, leave literal
                out.push('{');
                out.push_str(&spec);
                out.push('}');
            }
        } else {
            // unrecognized spec, leave literal
            out.push('{');
            out.push_str(&spec);
            out.push('}');
        }
    }

    out
}
