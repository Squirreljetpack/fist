# Clipboard

Text and file copies leave the process by one of two backends: the local
pasteboard (arboard) or an OSC 52 escape sequence written to a terminal. OSC 52
has no acknowledgement, so a copy can only be reported as successful when the
sequence was handed to a terminal that cannot be a captured stream. That
invariant — not "the bytes were written" — is what the delivery code below
enforces.

## Flow

`CopyPath`, `CopyFiles`, `CopyText` and the pager all end in
`copy_paths` / `copy_text` (`src/clipboard.rs`):

1. Bump `SEQ`; the copy runs on `TASKS::spawn_blocking`, so a slow pasteboard or
   bitmap build never blocks the event loop.
2. Take `WRITE_LOCK`; bail out if a newer copy has raised `SEQ` (the later copy
   wins, the earlier one is dropped silently).
3. `write` applies the trailing-newline policy and picks the backend.
4. `report` toasts success, or logs and toasts the failure. An OSC 52 write also
   requests a repaint: the escape sequence bypassed the TUI stream.

Text payload for paths is newline-delimited with no final newline
(`serialize_paths`); `tui.copy_trailing_newline` (default `false`) decides
whether that newline survives. `copy_from_pager` is the exception: it runs
synchronously on the pager thread and only logs failures.

## Backends

`clipboard::init` is called once at startup with `tui.osc52` and
`tui.copy_trailing_newline`.

- `tui.osc52 = true` (default) — the arboard handle is never constructed. It is
  the library's contract that a user of `matchmaker-lib` implements OSC 52
  itself, so `fs` does. Every text copy is an escape sequence and `get_text`
  returns `None`. `Payload::Files` (Copy / Move / CopyFiles) never falls back to
  OSC 52 and always attempts setting the OS file clipboard directly.
- `tui.osc52 = false` — arboard is tried at startup (X11 unconditionally,
  Wayland data-control only when `$WAYLAND_DISPLAY` is set); a failed init is
  logged and text copies fall back to OSC 52. With a live handle, file copies
  go through the file clipboard and text copies stay local.

The choice is a deployment property, not a per-copy one: a session without
`$DISPLAY` and without `$WAYLAND_DISPLAY` gains nothing from `osc52 = false`
except a logged init failure.

## OSC 52 delivery

### Framing

```
bare     \x1b]52;c;<base64 text>\x07
wrapped  \x1bPtmux;\x1b\x1b]52;c;<base64 text>\x07\x1b\\
```

`sequence` (`src/utils/osc52.rs`) builds both. The inner OSC 52 ends with `BEL`; the tmux wrapper ends with `ST` (`\x1b\\`). A multiplexer may re-emit the inner sequence with `ST` instead of `BEL` — both terminators are valid.

The wrapped form is a DCS addressed to tmux: tmux strips `Ptmux;`, un-doubles
`\x1b\x1b` and passes the inner OSC 52 to its client terminal. A terminal that
receives the wrapped form without tmux in the path discards the whole DCS as an
unimplemented command — silently, with no reply. That asymmetry is the reason
the wrapper follows the chosen target rather than `$TMUX` alone.

### Target selection

`write_osc52` tries, in order, stopping at the first target that accepts the
write:

| # | target                                                           | framing                                          |
| - | ---------------------------------------------------------------- | ------------------------------------------------ |
| 1 | `/dev/tty` (controlling terminal; the pane inside a multiplexer) | wrapped iff `$TMUX` is set                       |
| 2 | `$SSH_TTY`                                                       | always bare — the write bypasses the multiplexer |
| 3 | stdout, only when it is a terminal                               | wrapped iff `$TMUX` is set                       |
| 4 | nothing left                                                     | `Err`, surfaced as a failed copy                 |

`/dev/tty` is first because it is always the process's own terminal, while
`$SSH_TTY` can be stale, recycled, or foreign: inside tmux it is the tmux
*server's* environment, captured when the server started, because `SSH_TTY` is
not in tmux's `update-environment`. Reaching it at all only happens without a
controlling terminal.

Each attempt logs at debug level with the path and the error, and a copy that
finds no terminal returns an error instead of pretending: a success toast now
means the sequence left the process toward a terminal, never toward a pipe.

## Failure catalogue

### Unopenable `$SSH_TTY`

A login helper can allocate the pty as root and never chown the slave node, so
the login shell holds the fd but cannot `open()` the path. Tailscale SSH
(`tailscaled be-child ssh`) does this: `$SSH_TTY` is `crw------- root tty`.
When that path was tried first, the open failed and the old code fell back to
stdout — invisible in a direct invocation (stdout is the terminal) and fatal
inside a shell widget (`stdout` is the `$(...)` capture, so the sequence lands
in the result text instead of the terminal). Measured with the widget's stdout
on a pipe and `$SSH_TTY` on that node: the sequence appeared on the pipe only;
after the target order change it appears on the tty and the pipe receives zero
bytes.

### Framing the wrong transport

Keying the wrapper on `$TMUX` while writing to `$SSH_TTY` produces the wrapped
form outside any multiplexer, which the outer terminal drops. The pairing that
works: wrapped for bytes entering a pane, bare for bytes that skip the
multiplexer.

### stdout capture is only a symptom

A captured stdout changes nothing while a real terminal accepts the write. It
becomes fatal only through the stdout fallback, which is why the fallback is
restricted to a stdout that is itself a terminal.

## Multiplexer behaviour (measured)

Counts are deliveries of the exact sequence at the outer terminal, driven from a
pty that stands in for the client terminal. tmux 3.8, zellij 0.45.1, three
attempts per route.

| route                    | zellij                    | tmux defaults (`external`, passthrough off) | `allow-passthrough on` | `set-clipboard on`, passthrough off |
| ------------------------ | ------------------------- | ------------------------------------------- | ---------------------- | ----------------------------------- |
| wrapped DCS → pane       | n/a                       | 0/3                                         | 3/3                    | 0/3                                 |
| bare OSC 52 → pane       | 3/3, re-emitted with `ST` | 0/3                                         | 0/3                    | 3/3                                 |
| bare OSC 52 → `$SSH_TTY` | 6/6                       | 3/3                                         | 3/3                    | —                                   |

- zellij forwards a pane's OSC 52 to its client and needs no wrapper; the
  bypass route works too, straight into the client's stream.
- tmux `set-clipboard external` (the default since 3.3) neither sets the
  clipboard nor forwards an application's request, and `allow-passthrough off`
  (the default) discards the DCS. Both channels are therefore closed on a stock
  tmux; one of the two options has to be enabled.
- Under tmux, a bare write to `$SSH_TTY` works regardless of either option,
  because the sequence never enters tmux.

## Remaining gaps

- Stock tmux. Neither channel is open, so a pane copy is lost. tmux does know
  the answer: from inside a pane `tmux display -p '#{client_tty}'` reports the
  attached client's terminal, and a bare write there landed 3/3 under default
  options. It costs a subprocess per copy and is ambiguous with several clients
  attached, so it is not implemented.
- `screen` has no passthrough handling; the sequence is written bare to the
  pane and screen's own clipboard support decides.
- Windows has no `/dev/tty` and no `$SSH_TTY`; delivery is stdout, and a
  captured stdout fails loudly instead of silently.
- OSC 52 carries text only; file copies (`Payload::Files`) bypass OSC 52 and
  always attempt setting the OS file clipboard directly.

## Diagnosing

Write the sequence yourself, bypassing the application, and paste:

```shell
printf '\e]52;c;%s\a' "$(printf direct-probe | base64)" > /dev/tty
printf '\e]52;c;%s\a' "$(printf ssh-probe | base64)" > "$SSH_TTY"
```

If neither probe sticks, the terminal or the client side drops OSC 52 and no
change inside the process will help. If one sticks and a copy does not, compare
the target: the second command fails with `permission denied` exactly when the
node is not openable. `RUST_LOG=fist=debug fs` logs the chosen target and every
failed attempt.

## Sources

| topic                                             | file                                                              |
| ------------------------------------------------- | ----------------------------------------------------------------- |
| copy flow, backends, reporting                    | `src/clipboard.rs`                                                |
| sequence framing, target selection                | `src/utils/osc52.rs`                                              |
| copy actions                                      | `src/run/action.rs` (`fsaction_handler`)                          |
| startup wiring of `tui.osc52`                     | `src/run/start.rs`                                                |
| `tui.osc52`, `tui.copy_trailing_newline` defaults | `matchmaker-lib/src/config.rs` (`TerminalConfig`)                 |
| the same delivery logic in `mm`                   | `matchmaker-cli/src/register.rs` (`set_host_clipboard_universal`) |
