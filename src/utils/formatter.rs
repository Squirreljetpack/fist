//! Item templates: `{…}` placeholders expanded against the current item, the
//! selection, or a single path. Grammar reference: `docs/TEMPLATING.md`.

use std::{
    borrow::Cow,
    path::{MAIN_SEPARATOR, Path, PathBuf},
};

use cba::bring::consume_escaped;

use crate::{
    abspath::AbsPath,
    aliases::MMState,
    cli::paths::{__cwd, __home},
    run::item::PathItem,
};

// ----------------------------------------------------------------- HOSTS

/// One item under a template: the data a placeholder can consume.
struct Ctx<'a> {
    /// nucleo index of the item, `None` outside a live result list.
    index: Option<u32>,
    /// The "line": the value of `{}` and the base of path slices.
    path: Cow<'a, str>,
    /// The tail column. `None` wherever there is no item behind the path
    /// (listing, lessfilter, prompt) and while the results cursor is disabled.
    tail: Option<Cow<'a, str>>,
}

trait Host {
    fn current(&self) -> Option<Ctx<'_>>;
    /// Selected items in selection order, falling back to the current item when
    /// nothing is selected.
    fn selection(&self) -> Vec<Ctx<'_>>;
    /// Base of `.` slices.
    fn cwd(&self) -> Cow<'_, str>;
}

struct StateHost<'a, 'b> {
    state: &'a MMState<'b>,
    /// Overrides `{}` with the resolved execution target (a parent directory or
    /// the cwd while the cursor is disabled) while the tail still comes from
    /// the current item.
    target: Option<&'a AbsPath>,
}

impl StateHost<'_, '_> {
    fn ctx_of<'s>(
        &'s self,
        index: u32,
        item: &'s PathItem,
    ) -> Ctx<'s> {
        Ctx {
            index: Some(index),
            path: item.path.to_string_lossy(),
            tail: Some(item.tail_raw()),
        }
    }
}

impl Host for StateHost<'_, '_> {
    fn current(&self) -> Option<Ctx<'_>> {
        match self.state.picker_ui.current_indexed() {
            Some((index, item)) => Some(match self.target {
                Some(target) => Ctx {
                    path: target.to_string_lossy(),
                    ..self.ctx_of(index, item)
                },
                None => self.ctx_of(index, item),
            }),
            // cursor disabled: an execution target (the cwd) still resolves, but
            // there is no item behind it
            None => self.target.map(|target| Ctx {
                index: None,
                path: target.to_string_lossy(),
                tail: None,
            }),
        }
    }

    fn selection(&self) -> Vec<Ctx<'_>> {
        let mut out: Vec<Ctx> = self
            .state
            .picker_ui
            .selector
            .iter()
            .filter_map(|&idx| {
                let item = self.state.picker_ui.worker.get_by_idx(idx)?;
                Some(self.ctx_of(idx, item))
            })
            .collect();
        if out.is_empty() {
            out.extend(self.current());
        }
        out
    }

    fn cwd(&self) -> Cow<'_, str> {
        __cwd().to_string_lossy()
    }
}

/// Single-path host: output printing, listing, lessfilter, pager and the cwd
/// prompt. Only `{t}`/`{tail}` needs an item, so the item-less contexts (listing,
/// lessfilter, prompt) leave those literal, and `{#}` is always literal here.
struct PathHost<'a> {
    path: Cow<'a, str>,
    tail: Option<Cow<'a, str>>,
    /// Base of `.` slices.
    cwd: Cow<'a, str>,
}

impl Host for PathHost<'_> {
    fn current(&self) -> Option<Ctx<'_>> {
        Some(Ctx {
            index: None,
            path: Cow::Borrowed(self.path.as_ref()),
            tail: self.tail.as_ref().map(|t| Cow::Borrowed(t.as_ref())),
        })
    }

    fn selection(&self) -> Vec<Ctx<'_>> {
        self.current().into_iter().collect()
    }

    fn cwd(&self) -> Cow<'_, str> {
        Cow::Borrowed(self.cwd.as_ref())
    }
}

// ----------------------------------------------------------------- PARSER

/// What a placeholder reads. The single delimiter picks base, operation and
/// quoting, so there are no prefixes and no combinations.
enum Body {
    /// `+`: one value per selected item, joined.
    Each(Each),
    /// A single value, from the current item.
    Value(Value),
}

/// The per-item value of a `+` spec.
enum Each {
    /// `{+}` / `{S+E}`: component window of the path.
    Window { start: i32, end: i32 },
    /// `{+t}` / `{+tail}`: the tail column.
    Tail,
    /// `{+#}`: the item index.
    Index,
}

impl Each {
    /// Only the bare multi window is a shell argument.
    fn raw(&self) -> bool {
        matches!(self, Each::Tail | Each::Index)
    }
}

/// A single value: the line, a component range, or a column.
enum Value {
    /// `{}`: the line.
    Line,
    /// `{S:E}` quoted, `{S=E}` raw.
    Slice { raw: bool, start: i32, end: i32 },
    /// `{S.E}`: component range of the current directory.
    Cwd { start: i32, end: i32 },
    /// `{S~E}`: keep the first `S` and last `E` components.
    Truncate { start: i32, end: i32 },
    /// `{t}` / `{tail}`: the tail column.
    Tail,
    /// `{#}`: the item index.
    Index,
}

impl Value {
    /// Path slices are shell arguments and quoted by default; `=` and the
    /// display forms are raw, and so are the columns (they read data, not a
    /// path).
    fn raw(&self) -> bool {
        match self {
            Value::Line => false,
            Value::Slice { raw, .. } => *raw,
            Value::Cwd { .. } | Value::Truncate { .. } | Value::Tail | Value::Index => true,
        }
    }
}

fn parse_spec(spec: &str) -> Option<Body> {
    match spec {
        "" => return Some(Body::Value(Value::Line)),
        "t" | "tail" => return Some(Body::Value(Value::Tail)),
        "#" => return Some(Body::Value(Value::Index)),
        _ => {}
    }

    let (start, delim, end) = split_on_first_delim(spec, [':', '=', '.', '+', '~'])?;

    Some(match delim {
        // multi: a column name on the right of the delimiter, otherwise a
        // component window of every selected path
        '+' => Body::Each(match end {
            "t" | "tail" if start.is_empty() => Each::Tail,
            "#" if start.is_empty() => Each::Index,
            _ => Each::Window {
                start: parse_int(start)?,
                end: parse_int(end)?,
            },
        }),
        ':' | '=' => Body::Value(Value::Slice {
            raw: delim == '=',
            start: parse_int(start)?,
            end: parse_int(end)?,
        }),
        '.' => Body::Value(Value::Cwd {
            start: parse_int(start)?,
            end: parse_int(end)?,
        }),
        '~' => {
            let start = parse_int(start)?;
            let end = parse_int(end)?;
            // "first N … last N components" has no negative form
            if start < 0 || end < 0 {
                return None;
            }
            Body::Value(Value::Truncate { start, end })
        }
        _ => return None,
    })
}

fn parse_int(s: &str) -> Option<i32> {
    if s.is_empty() {
        Some(0)
    } else if s.starts_with('+') {
        // `+` is the multi delimiter, never a sign
        None
    } else {
        s.parse().ok()
    }
}

/// Splits at the first occurrence of any of `delims`, returning the delimiter.
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

// ----------------------------------------------------------------- RENDER

fn render(
    host: &impl Host,
    template: &str,
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

        let mut spec = String::new();
        let mut closed = false;
        while let Some(&c) = chars.peek() {
            if c == '}' {
                chars.next();
                closed = true;
                break;
            }
            spec.push(c);
            chars.next();
        }

        let rendered = closed
            .then(|| parse_spec(&spec))
            .flatten()
            .and_then(|spec| render_spec(host, &spec));

        match rendered {
            Some(value) => out.push_str(&value),
            None => {
                // unrecognized spec: leave it literal
                out.push('{');
                out.push_str(&spec);
                if closed {
                    out.push('}');
                }
            }
        }
    }

    out
}

fn render_spec(
    host: &impl Host,
    body: &Body,
) -> Option<String> {
    let value = match body {
        Body::Each(each) => {
            // one value per selected item, joined. A multi spec never falls back
            // to the literal text: callers read an empty command as "nothing to
            // do", and the host falls back to the current item when there is no
            // selection at all
            return Some(
                host.selection()
                    .iter()
                    .filter_map(|ctx| each_text(ctx, each))
                    .map(|text| quote(each.raw(), text))
                    .collect::<Vec<_>>()
                    .join(" "),
            );
        }
        Body::Value(value) => value,
    };

    let Some(ctx) = host.current() else {
        // no item at all (preview with the cursor disabled): render empty so
        // callers see "nothing to do" rather than a literal template
        return Some(String::new());
    };

    let text = match value {
        Value::Line => ctx.path.as_ref().to_owned(),
        Value::Tail => ctx.tail.as_ref()?.as_ref().to_owned(),
        Value::Index => ctx.index?.to_string(),
        Value::Slice { start, end, .. } => slice_text(&ctx.path, *start, *end),
        Value::Cwd { start, end } => slice_text(&host.cwd(), *start, *end),
        Value::Truncate { start, end } => {
            truncate_path(Path::new(ctx.path.as_ref()), *start as usize, *end as usize)
        }
    };

    Some(quote(value.raw(), text))
}

fn each_text(
    ctx: &Ctx<'_>,
    each: &Each,
) -> Option<String> {
    match each {
        Each::Window { start, end } => Some(slice_text(&ctx.path, *start, *end)),
        Each::Tail => ctx.tail.as_ref().map(|tail| tail.as_ref().to_owned()),
        Each::Index => ctx.index.map(|index| index.to_string()),
    }
}

/// `start..end` component window of `text`, as a path string.
fn slice_text(
    text: &str,
    start: i32,
    end: i32,
) -> String {
    slice_path(Path::new(text), start, end)
        .to_string_lossy()
        .into_owned()
}

/// Shell-quote a value, with embedded `'` escaped as `'\''`.
fn quote(
    raw: bool,
    value: String,
) -> String {
    if raw {
        value
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

fn slice_path(
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

fn truncate_path(
    path: &Path,
    start: usize,
    end: usize,
) -> String {
    let (first, last) = split_path_ends(path, start, end);
    if last.is_empty() {
        first
    } else {
        format!("{first}…{MAIN_SEPARATOR}{last}")
    }
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

// ----------------------------------------------------------------- PUBLIC

/// State-based template: the current item, the selection and the tail column.
pub fn format_state(
    template: &str,
    state: &MMState<'_>,
) -> String {
    render(
        &StateHost {
            state,
            target: None,
        },
        template,
    )
}

/// State-based template with `{}` resolved to an execution target that is not
/// the current item's own path (a parent directory, or the cwd while the cursor
/// is disabled).
pub fn format_state_at(
    template: &str,
    state: &MMState<'_>,
    target: &AbsPath,
) -> String {
    render(
        &StateHost {
            state,
            target: Some(target),
        },
        template,
    )
}

/// Single-item template for per-item output printing: the item is the line, so
/// `{t}`/`{tail}` resolve for every printed item, but `{#}` (results index) does
/// not exist outside a live result list and stays literal.
pub fn format_item(
    template: &str,
    item: &PathItem,
) -> String {
    render(
        &PathHost {
            path: item.path.to_string_lossy(),
            tail: Some(item.tail_raw()),
            cwd: __cwd().to_string_lossy(),
        },
        template,
    )
}

/// Template the matchmaker preview seam accepts
/// ([`matchmaker::AttachmentFormatter`]'s state-aware variant).
pub fn preview_formatter(
    state: &MMState<'_>,
    template: &str,
    _repeat: Option<&dyn Fn(String)>,
) -> String {
    format_state(template, state)
}

/// Single-path template for contexts without a result list (listing,
/// lessfilter, pager).
pub fn format_path(
    template: &str,
    path: &AbsPath,
) -> String {
    render(
        &PathHost {
            path: path.to_string_lossy(),
            tail: None,
            cwd: __cwd().to_string_lossy(),
        },
        template,
    )
}

/// The cwd prompt: the path is home-collapsed. Quoting works as everywhere else,
/// so `{=}` is the bare form.
pub fn format_prompt(
    template: &str,
    cwd: &Path,
) -> String {
    let path: Cow<'_, str> = match cwd.strip_prefix(__home()) {
        Ok(stripped) => Cow::Owned(
            PathBuf::from("~")
                .join(stripped)
                .to_string_lossy()
                .into_owned(),
        ),
        Err(_) => cwd.to_string_lossy(),
    };
    render(
        &PathHost {
            cwd: path.clone(),
            path,
            tail: None,
        },
        template,
    )
}

// ----------------------------------------------------------------- TESTS

#[cfg(test)]
mod tests {
    use super::*;

    const P: &str = "/home/you/docs/report.md";

    fn path(template: &str) -> String {
        render(
            &PathHost {
                path: Cow::Borrowed(P),
                tail: None,
                cwd: Cow::Borrowed("/home/you"),
            },
            template,
        )
    }

    fn tied(
        template: &str,
        item: &PathItem,
        index: Option<u32>,
    ) -> String {
        struct TestHost<'a> {
            item: &'a PathItem,
            index: Option<u32>,
        }
        impl Host for TestHost<'_> {
            fn current(&self) -> Option<Ctx<'_>> {
                Some(Ctx {
                    index: self.index,
                    path: self.item.path.to_string_lossy(),
                    tail: Some(self.item.tail_raw()),
                })
            }
            fn selection(&self) -> Vec<Ctx<'_>> {
                self.current().into_iter().collect()
            }
            fn cwd(&self) -> Cow<'_, str> {
                Cow::Borrowed("/home/you")
            }
        }
        render(&TestHost { item, index }, template)
    }

    fn multi<'a>(
        paths: &'a [&'a str],
        template: &str,
    ) -> String {
        struct TestHost<'a> {
            paths: &'a [&'a str],
        }
        impl Host for TestHost<'_> {
            fn current(&self) -> Option<Ctx<'_>> {
                self.paths.first().map(|p| Ctx {
                    index: Some(0),
                    path: Cow::Borrowed(p),
                    tail: None,
                })
            }
            fn selection(&self) -> Vec<Ctx<'_>> {
                self.paths
                    .iter()
                    .enumerate()
                    .map(|(i, p)| Ctx {
                        index: Some(i as u32),
                        path: Cow::Borrowed(p),
                        tail: None,
                    })
                    .collect()
            }
            fn cwd(&self) -> Cow<'_, str> {
                Cow::Borrowed("/home/you")
            }
        }
        render(&TestHost { paths }, template)
    }

    #[test]
    fn line_and_slices() {
        assert_eq!(path("{}"), format!("'{P}'"));
        assert_eq!(path("{=}"), P);
        assert_eq!(path("{0:-1}"), "'/home/you/docs'");
        assert_eq!(path("{-1:0}"), "'report.md'");
        assert_eq!(path("{-1=0}"), "report.md");
        assert_eq!(path("{1:3}"), "'home/you'");
        assert_eq!(path("{0=0}"), P);
        assert_eq!(path("{:}"), format!("'{P}'"));
        assert_eq!(path("{100:0}"), "''");
        assert_eq!(path("{-100:0}"), format!("'{P}'"));
        assert_eq!(path("{0=-1}"), "/home/you/docs");
        assert_eq!(path("{=}"), P);
    }

    #[test]
    fn cwd_slices() {
        assert_eq!(path("{1.3}"), "home/you");
        assert_eq!(path("{.}"), "/home/you");
    }

    #[test]
    fn quotes_and_escapes() {
        let p = PathHost {
            path: Cow::Borrowed("/tmp/it's here"),
            tail: None,
            cwd: Cow::Borrowed("/tmp"),
        };
        assert_eq!(render(&p, "{}"), r"'/tmp/it'\''s here'");
        assert_eq!(render(&p, "{=}"), "/tmp/it's here");
        assert_eq!(render(&p, r"\{}\n"), "{}\n".replace("\\n", "\n"));
        assert_eq!(render(&p, "a\\tb"), "a\tb");
        assert_eq!(render(&p, r"\{tail}"), "{tail}");
    }

    #[test]
    fn tail_column() {
        let mut item = PathItem::new_unchecked(PathBuf::from(P));
        item.tail = Ok(["12:3: fn main() {".to_string(), "Override".to_string()]);
        // state host (commands, previews: an item plus its result index)
        assert_eq!(tied("{t}", &item, Some(7)), "12:3: fn main() {");
        assert_eq!(tied("{tail}", &item, Some(7)), "12:3: fn main() {");
        // the display override never surfaces through the tail key
        assert!(!tied("{t}", &item, Some(7)).contains("Override"));
        assert_eq!(tied("{#}", &item, Some(7)), "7");
        // multi over one item: the per-item columns, raw
        assert_eq!(tied("{+t}", &item, Some(7)), "12:3: fn main() {");
        assert_eq!(tied("{+tail}", &item, Some(7)), "12:3: fn main() {");
        assert_eq!(tied("{+#}", &item, Some(7)), "7");
        assert_eq!(tied("{+}", &item, Some(7)), format!("'{P}'"));

        // per-item output printing: the item is the line, no result index
        assert_eq!(format_item("{t}", &item), "12:3: fn main() {");
        assert_eq!(format_item("{-1=0}", &item), "report.md");
        assert_eq!(format_item("{}", &item), format!("'{P}'"));
        assert_eq!(format_item("{#}", &item), "{#}");

        // item-less contexts: both literal
        assert_eq!(path("{t}"), "{t}");
        assert_eq!(path("{tail}"), "{tail}");
        assert_eq!(path("{#}"), "{#}");
    }

    #[test]
    fn multi_selection() {
        let paths = ["a/first.txt", "b/second.txt"];
        assert_eq!(multi(&paths, "{+}"), "'a/first.txt' 'b/second.txt'");
        assert_eq!(multi(&paths, "{0+1}"), "'a' 'b'");
        assert_eq!(multi(&paths, "{-1+0}"), "'first.txt' 'second.txt'");
        assert_eq!(multi(&paths, "{0+0}"), "'a/first.txt' 'b/second.txt'");
        assert_eq!(multi(&[], "{+}"), "");
        assert_eq!(multi(&paths, "{+.}"), "{+.}");
        // one delimiter per spec: no raw multi
        assert_eq!(multi(&paths, "{+=}"), "{+=}");
        assert_eq!(multi(&paths, "{+0:1}"), "{+0:1}");
        // per-item columns: indexes here, tails absent from this host
        assert_eq!(multi(&paths, "{+#}"), "0 1");
        assert_eq!(multi(&paths, "{+t}"), "");
    }

    #[test]
    fn truncation() {
        assert_eq!(path("{2~2}"), "/home…/docs/report.md");
        assert_eq!(path("{~2}"), "…/docs/report.md");
        assert_eq!(path("{2~}"), "/home");
        assert_eq!(path("{9~9}"), P);
        // truncation with no components left is empty, not the whole path
        assert_eq!(path("{~}"), "");
        assert_eq!(path("{1~1}"), "/…/report.md");
        assert_eq!(path("{~a:2}"), "{~a:2}");
    }

    #[test]
    fn prompt_quotes_and_truncates() {
        let cwd = Path::new("/zzz/q/w/e/r");
        assert_eq!(format_prompt("{}", cwd), "'/zzz/q/w/e/r'");
        assert_eq!(format_prompt("{=}", cwd), "/zzz/q/w/e/r");
        assert_eq!(format_prompt("{2~2}", cwd), "/zzz…/e/r");
        assert_eq!(format_prompt("{2:2}", cwd), "''");
        assert_eq!(format_prompt("{1:3}", cwd), "'zzz/q'");
        // `{S.E}` is a slice of the collapsed cwd, `{S~E}` a truncation of it
        assert_eq!(format_prompt("{1.3}", cwd), "zzz/q");
        assert_eq!(format_prompt("{t}", cwd), "{t}");
    }

    #[test]
    fn prompt_collapses_home() {
        let cwd = __home().join("a/b/c");
        assert_eq!(format_prompt("{=}", &cwd), "~/a/b/c");
        assert_eq!(format_prompt("{1~1}", &cwd), "~…/c");
    }

    #[test]
    fn invalid_specs_stay_literal() {
        for template in [
            "{foo}", "{!}", "{1..3}", "{tail:0}", "{%}", "{=t}", "{=tail}", "{=#}", "{=0:0}",
            "{=0:-1}", "{+:}", "{0+~1}", "{-1~0}", "{1..}", "{-}", "{t+}", "{+0+1}",
        ] {
            assert_eq!(path(template), template, "{template}");
        }
        assert_eq!(path("{"), "{");
        assert_eq!(path("{unclosed"), "{unclosed");
    }
}
