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
