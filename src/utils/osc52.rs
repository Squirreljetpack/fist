//! OSC 52 clipboard writes: port of matchmaker-cli's
//! `set_host_clipboard_universal`. The escape sequence goes to a terminal
//! device rather than the TUI output stream, which a shell widget may capture.

use std::{
    env,
    fs::OpenOptions,
    io::{self, IsTerminal, Write},
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
///
/// The sequence is written to the first terminal device that accepts it: our
/// controlling terminal, then `$SSH_TTY`, then stdout when it is a terminal.
/// The first two are unreachable by a capturing stdout redirect, so the copy
/// cannot be silently swallowed; a stdout that is not a terminal fails instead
/// of reporting a success that never reached a terminal.
pub fn write_osc52(text: &str) -> io::Result<()> {
    // A tmux passthrough is only understood on the way into a pane. A write
    // that bypasses the multiplexer must stay bare: the outer terminal does
    // not speak the DCS wrapper.
    let pane_seq = sequence(text, env::var("TMUX").is_ok());
    let bare_seq = sequence(text, false);

    let mut targets = vec![("/dev/tty", &pane_seq)];
    let ssh_tty = env::var("SSH_TTY").ok();
    if let Some(path) = ssh_tty.as_deref() {
        targets.push((path, &bare_seq));
    }

    let mut last_err = None;
    for (path, seq) in targets {
        match write_to(path, seq) {
            Ok(()) => {
                log::debug!("OSC 52 clipboard write to {path}");
                return Ok(());
            }
            Err(e) => {
                log::debug!("OSC 52 clipboard write to {path} failed: {e}");
                last_err = Some(e);
            }
        }
    }

    if io::stdout().is_terminal() {
        return write_into(io::stdout(), &pane_seq);
    }
    Err(last_err.unwrap_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "no terminal accepted the OSC 52 sequence",
        )
    }))
}

fn write_to(
    path: &str,
    seq: &str,
) -> io::Result<()> {
    write_into(OpenOptions::new().write(true).open(path)?, seq)
}

fn write_into(
    mut sink: impl Write,
    seq: &str,
) -> io::Result<()> {
    sink.write_all(seq.as_bytes())?;
    sink.flush()
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
        assert_eq!(
            sequence("hi", true),
            "\x1bPtmux;\x1b\x1b]52;c;aGk=\x07\x1b\\"
        );
    }
}
