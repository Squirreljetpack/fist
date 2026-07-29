use std::io::Write;
use std::time::Duration;
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::time::timeout;

pub const READ_TIMEOUT: Duration = const { Duration::from_secs(60) }; // a bit weird but i don't want trash to last forever

/// Prompts the user with a timeout. The default choice is autodetected from the
/// first option starting with an uppercase letter (e.g., `["yes", "No"]`).
/// Returns `None` on timeout, otherwise `Some(true)` if index 0 is chosen.
/// Prompts via stdin with a timeout. Default is the capitalized option.
/// Returns `None` on timeout, otherwise `Some(usize)` of the selected index.
pub async fn confirm_prompt(
    msg: &str,
    dt: Duration,
    opts: Vec<&'static str>,
) -> Option<usize> {
    let def = opts
        .iter()
        .position(|o| o.starts_with(char::is_uppercase))
        .unwrap_or(0);
    let prompt = format!("{} [{}]: ", msg, opts.join("/"));
    let (mut stdin, mut stdout, mut line) =
        (BufReader::new(io::stdin()), io::stdout(), String::new());

    loop {
        let _ = stdout.write_all(prompt.as_bytes()).await;
        let _ = stdout.flush().await;
        line.clear();

        match timeout(dt, stdin.read_line(&mut line)).await {
            Ok(Ok(n)) if n > 0 => {
                let s = line.trim().to_lowercase();
                if s.is_empty() {
                    return Some(def);
                }
                if let Some(i) = opts.iter().position(|o| o.to_lowercase().starts_with(&s)) {
                    return Some(i);
                }
            }
            Ok(_) => return Some(def),
            Err(_) => return None,
        }
    }
}

#[allow(unused)]
/// Prompts via stdin synchronously. Default is the capitalized option.
/// Returns the `usize` index of the selected option.
pub fn confirm_prompt_sync(
    msg: &str,
    opts: Vec<&'static str>,
) -> usize {
    let def = opts
        .iter()
        .position(|o| o.starts_with(char::is_uppercase))
        .unwrap_or(0);
    let prompt = format!("{} [{}]: ", msg, opts.join("/"));
    let mut line = String::new();

    loop {
        print!("{}", prompt);
        let _ = std::io::stdout().flush();
        line.clear();

        if std::io::stdin().read_line(&mut line).is_ok() {
            let s = line.trim().to_lowercase();
            if s.is_empty() {
                return def;
            }
            if let Some(i) = opts.iter().position(|o| o.to_lowercase().starts_with(&s)) {
                return i;
            }
        } else {
            return def;
        }
    }
}
