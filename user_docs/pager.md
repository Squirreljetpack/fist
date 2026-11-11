# The Pager

`fs` includes a custom pager built on [minus](https://github.com/AMythicDev/minus). This pager includes a few modifications required by `fs` for a smooth experience: it wraps [bat](https://github.com/sharkdp/bat) when present, skips starting on empty (non-file) inputs, writes to /dev/tty in `fist` (so that `fs` can still write to stdout), and reads environment variables to propogate options such as bat arguments so that it fits as a configurable `$PAGER` in scripts (if so desired) with maximal compatibility.

## When output gets paged

- **Actions**: `ExecPaged` and`LfPaged` run the command with its stdout streamed into an interactive
  minus pager drawn on `/dev/tty`. Quitting the pager (`q`) kills the command
  and returns to the TUI.
- **Lessfilter**: The `preview`, `display` and `extract` presets render a compatible files' content into the preview pane.
- **`fs :tool pager`**: Behaves similarly to standard pagers, but provides syntax highlighting through bat, and reads options through environment variables.

## Configuration — `pager.toml`

```toml
bat_opts = ["--color=always", "--style=changes"]
line_numbers = false
horizontal_scroll = false
smart_case = true
# prompt = "" # disable prompt text
```

| Field               | Type            | Meaning                                                                                               |
| ------------------- | --------------- | ----------------------------------------------------------------------------------------------------- |
| `bat_opts`          | list of strings | Extra arguments passed to `bat`.                                                                      |
| `line_numbers`      | bool            | Show line numbers in the pager.                                                                       |
| `horizontal_scroll` | bool            | Always enable horizontal scrolling (Ctrl+h still toggles it).                                         |
| `prompt`            | string          | Footer prompt text shown by the pager.                                                                |
| `smart_case`        | bool            | Queries with no uppercase chars match case-insensitively; queries with uppercase stay case-sensitive. |

`bat` is only used when the `bat` binary is on `PATH`; without it, output
passes through uncolored.

## `fs :tool pager` (alias `pg`)

Page one file, or stdin when no file is given:

```sh
fs :tool pager Cargo.toml
cat Cargo.toml | fs :tool pager
fs :tool pager -- Cargo.toml
```

- Bat arguments come from the environment (see below) — the same options the
  lessfilter renderer honors.
- Empty non-file input produces skips the pager and exits 0.
- On a terminal, output is paged interactively (minus), with bat colors when
  bat is installed. When stdout is a pipe or file, content passes through
  with colors but no paging UI.

### Opening at a position — `+OFFSET`

An argument starting with `+` chooses where the pager opens:

| Form  | Meaning                                                                                       |
| ----- | --------------------------------------------------------------------------------------------- |
| `+N`  | Open with line `N` (1-based) at the top of the display; the rest of the file stays available. |
| `+Np` | Percent form of the above, e.g. `+50p` for the middle of the file. Needs a file argument.     |
| `+F`  | Page the whole file, scrolled to the end, and follow it as it grows (`Ctrl+f` toggles).       |
| `+NF` | Page only the last `N` lines, then follow the file as it grows. Needs a file argument.        |

```sh
fs :tool pager +120 src/main.rs
fs :tool pager +50p src/main.rs
fs :tool pager +F /var/log/syslog
fs :tool pager +200F /var/log/syslog
```

With `-C NUM`, the display is padded with `NUM` lines of context around the
`+N` target line.

## Environment variables

These apply to `fs :tool pager` and every paged lessfilter output (`preview`,
`display`, `extract`):

| Variable         | Effect                                                                                                    |
| ---------------- | --------------------------------------------------------------------------------------------------------- |
| `PG_RAW=true`    | Skip bat entirely; pass the raw stream through.                                                           |
| `PG_FLAGS`       | Comma-separated bat flags that replace the default argument set (a doubled `,,` escapes a literal comma). |
| `PG_LANG`        | Bat language override: `-l <value>` (e.g. `PG_LANG=ini`).                                                 |
| `PG_FILENAME`    | Bat file name hint: `--file-name <value>` (used for language detection on stdin input).                   |
| `HIGHLIGHT_LINE` | Number: bat `--highlight-line <value>`.                                                                   |

### minus keybindings

These are the defaults of minus 5.7. A keybinding table for all of
minus's actions lives in the [minus documentation](https://docs.rs/minus/latest/minus/).

| Key                            | Action                                      |
| ------------------------------ | ------------------------------------------- |
| `q`, `Ctrl-c`                  | Quit the pager (always resumes the picker)  |
| `j` / `↓`, `k` / `↑`           | Scroll one line (prefix a number to repeat) |
| `Space`, `PageDown` / `PageUp` | Scroll one page                             |
| `u`, `Ctrl-u` / `d`, `Ctrl-d`  | Scroll half a page                          |
| `g` / `G`                      | Go to the top / bottom (or to line N)       |
| `Enter`                        | Scroll N lines                              |
| `h` / `←`, `l` / `→`           | Scroll horizontally                         |
| `Ctrl-l`                       | Toggle line numbers                         |
| `Ctrl-f`                       | Toggle follow mode                          |
| `Ctrl + H`                     | Toggle line wrap                            |
| `/`, `?`                       | Search forward / backward                   |
| `n`, `p`                       | Jump to the next / previous match           |

## Files

- `src/pager.rs` — paging core (`page_child`, `page_reader`, `render_text`).
- `src/lessfilter/helpers.rs` — `env_bat_opts()`: environment → bat arguments.
- `src/run/register/execute.rs` — paged execution plumbing.
