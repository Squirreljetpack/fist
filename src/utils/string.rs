use std::path::{MAIN_SEPARATOR, Path, PathBuf};

use cli_boilerplate_automation::bring::consume_escaped;

use crate::{
    abspath::AbsPath,
    cli::paths::{__cwd, __home},
};

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
    let cwd = if let Ok(stripped) = cwd.strip_prefix(__home()) {
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

/// - Split on whitespace
/// - maintain within '.
/// - \ escapes ' only.
pub fn split_whitespace_keep_single_quotes(s: &str) -> Vec<String> {
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
                    if next != '\'' {
                        cur.push('\\');
                    }
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

pub fn slice_path(
    path: &Path,
    start: i32,
    end: i32,
) -> PathBuf {
    let comps: Vec<_> = path.components().collect();
    let len = comps.len() as i32;

    let norm = |i: i32| {
        if i < 0 {
            (len + i).clamp(0, len)
        } else {
            i.clamp(0, len)
        }
    };

    let s = norm(start);
    let e = if end == 0 { len } else { norm(end) };

    comps[s as usize..e as usize]
        .iter()
        .fold(PathBuf::new(), |mut p, c| {
            p.push(c.as_os_str());
            p
        })
}

pub fn path_formatter(
    template: &str,
    path: &AbsPath,
) -> String {
    let mut out = String::with_capacity(template.len());
    let mut chars = template.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            consume_escaped(&mut chars, &mut out);
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
            out.push_str(&path.to_string_lossy().replace('\'', "'\\''"));
            out.push('\'');
        } else if let Some((a, d, b)) = split_on_first_delim(&spec, [':', '=', '.']) {
            // check if both a and b are integers
            let start = if a.is_empty() {
                Some(0)
            } else {
                a.parse::<i32>().ok()
            };
            let end = if b.is_empty() {
                Some(0)
            } else {
                b.parse::<i32>().ok()
            };
            if let (Some(start), Some(end)) = (start, end) {
                match d {
                    ':' => {
                        out.push('\'');
                        out.push_str(
                            &slice_path(path, start, end)
                                .to_string_lossy()
                                .replace('\'', "'\\''"),
                        );
                        out.push('\'');
                    }
                    '=' => {
                        out.push_str(&slice_path(path, start, end).to_string_lossy());
                    }
                    '.' => {
                        out.push_str(&slice_path(__cwd(), start, end).to_string_lossy());
                    }
                    _ => unreachable!(),
                }
            } else {
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

fn split_on_first_delim<const N: usize>(
    s: &str,
    delims: [char; N],
) -> Option<(&str, char, &str)> {
    let mut first: Option<(usize, char)> = None;

    for d in delims {
        if let Some(i) = s.find(d) {
            if first.is_none_or(|(j, _)| i < j) {
                first = Some((i, d));
            }
        }
    }

    let (i, d) = first?;
    Some((&s[..i], d, &s[i + d.len_utf8()..]))
}
