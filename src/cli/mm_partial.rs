use anyhow::bail;
use matchmaker::action::ArrayVec;

use thiserror::Error;
#[derive(Debug, Error)]
pub enum ParseError {
    #[error("Invalid path component '{component}' in path '{path}'")]
    InvalidPath { path: String, component: String },
    #[error("Missing value for path '{path}'")]
    MissingValue { path: String },
}

static ALIASES: &[(&str, &str)] = &[
    ("i", "start.input_separator"),
    ("o", "start.output_template"),
    ("x", "start.command"),
    ("cmd", "start.command"),
    ("a", "start.ansi"),
    ("t", "start.trim"),
    //
    ("d", "matcher.split"),
    //
    ("px", "preview.layout.command"),
    ("l", "preview.layout"),
    ("h", "header.content"),
];

/// Get (path, value) pairs by consuming either a single word, splitting at '=' into a valid key, or a pair of consecutive words.
/// The value is broken down into words, and fed into [`matchmaker_partial::Set`] to construct the (partial) type at `path`.
pub fn get_pairs(pairs: Vec<String>) -> Result<Vec<(ArrayVec<String, 10>, String)>, ParseError> {
    let mut result = Vec::new();
    let mut iter = pairs.into_iter().peekable();

    while let Some(item) = iter.next() {
        let (mut path_str, value) = if let Some(eq_pos) = item.find('=') {
            let path = item[..eq_pos].to_string();
            let val = item[eq_pos + 1..].to_string();

            // keep commented to allow empty value for setting bool as `m.ansi=`
            // if val.is_empty() {
            //     return Err(ParseError::MissingValue { path: path.clone() });
            // }
            (path, val)
        } else {
            let path = item;
            let val = iter
                .next()
                .ok_or_else(|| ParseError::MissingValue { path: path.clone() })?;
            (path, val)
        };

        // Apply alias to full path string
        if let Some((_, expanded)) = ALIASES.iter().find(|(a, _)| *a == path_str) {
            path_str = (*expanded).to_string();
        }

        let mut components = ArrayVec::<String, 10>::new();
        for comp in path_str.split('.') {
            if !valid_key(comp, false) {
                return Err(ParseError::InvalidPath {
                    path: path_str.clone(),
                    component: comp.to_string(),
                });
            }
            components.push(comp.to_string());
        }

        result.push((components, value));
    }

    Ok(result)
}

pub fn valid_key(
    s: &str,
    extended: bool,
) -> bool {
    !s.is_empty()
        && s.chars().all(|c| {
            (extended && c.is_ascii_graphic()) || c.is_ascii_lowercase() || "_".contains(c)
        })
}

/// Determine if a sequence of words should be interpreted as words representing key-value pairs (in which case they are split in two), or not (in which case the words are unchanged).
/// This is relevant to Maps and Structs as they are defined given a word sequences, interpreting it in word pairs.
pub fn try_split_kv(
    vec: &mut Vec<String>,
    extended_keys: bool,
) -> anyhow::Result<()> {
    // Check first element for '='
    if let Some(first) = vec.first()
        && let Some(pos) = first.find('=')
    {
        let key = &first[..pos];
        // If the first element is a valid k=v pair, split the rest, and require that they succeed
        if valid_key(key, extended_keys) {
            let mut out = Vec::with_capacity(vec.len() * 2);
            for s in vec.iter() {
                if let Some(pos) = s.find('=') {
                    let key = &s[..pos];
                    let val = &s[pos + 1..];
                    if !valid_key(key, extended_keys) {
                        bail!("Invalid key: {}", key);
                    }
                    out.push(key.to_string());
                    out.push(val.to_string());
                } else {
                    bail!("Expected '=' in element: {}", s);
                }
            }
            *vec = out;
        }
    }

    // otherwise no change
    Ok(())
}
