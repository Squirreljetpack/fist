//! OSC 52 clipboard writes: port of matchmaker-cli's
//! `set_host_clipboard_universal`. The escape sequence is written directly to
//! the controlling terminal (or `$SSH_TTY`) so the TUI output stream is left
//! untouched; when the TTY cannot be opened it falls back to stdout, wrapped
//! for tmux when `$TMUX` is set.

use std::{
    env,
    fs::OpenOptions,
    io::{self, Write},
};

/// Build the OSC 52 copy sequence for `text`; wrap it in a tmux passthrough
/// when `tmux` is true.
pub fn sequence(
    text: &str,
    tmux: bool,
) -> String {
    use base64::Engine as _;
    let encoded = base64::engine::general_purpose::STANDARD.encode(text);
    if tmux {
        format!("\x1bPtmux;\x1b\x1b]52;c;{encoded}\x07\x1b\\")
    } else {
        format!("\x1b]52;c;{encoded}\x07")
    }
}

/// Copy `text` to the host clipboard via OSC 52.
pub fn write_osc52(text: &str) -> io::Result<()> {
    // Over SSH, $SSH_TTY points at the exact device file; otherwise use the
    // process's controlling terminal.
    let tty_path = env::var("SSH_TTY").unwrap_or_else(|_| "/dev/tty".to_string());

    match OpenOptions::new().write(true).open(&tty_path) {
        Ok(mut tty_file) => {
            // A direct TTY write bypasses stdout and any multiplexer stream.
            tty_file.write_all(sequence(text, false).as_bytes())?;
            tty_file.flush()?;
        }
        Err(_) => {
            // Fallback when the TTY is unavailable (e.g. Windows): stdout,
            // wrapped for tmux if we are inside it.
            let seq = sequence(text, env::var("TMUX").is_ok());
            let mut stdout = io::stdout();
            stdout.write_all(seq.as_bytes())?;
            stdout.flush()?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_sequence_base64_encodes_payload() {
        assert_eq!(sequence("hello", false), "\x1b]52;c;aGVsbG8=\x07");
    }

    #[test]
    fn empty_payload_sequence() {
        assert_eq!(sequence("", false), "\x1b]52;c;\x07");
    }

    #[test]
    fn tmux_sequence_wraps_passthrough() {
        assert_eq!(sequence("hi", true), "\x1bPtmux;\x1b\x1b]52;c;aGk=\x07\x1b\\");
    }
}
