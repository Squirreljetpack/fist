# The Pager

`fs` includes a smart pager which wraps [bat](https://github.com/sharkdp/bat) when present, feeding it into [minus](https://github.com/AMythicDev/minus). This pager includes a few modifications required by `fs` for a smooth experience: it skips starting on empty (non-file) inputs, writes to /dev/tty in `fist` (so that `fs` can still write to stdout), and reads environment variables to propogate options such as bat arguments so that it fits as a configurable `$PAGER` in scripts (if so desired) with maximal compatibility.

## When output gets paged

- **Actions**: `ExecPaged` and`LfPaged` run the command with its stdout streamed into an interactive
  minus pager drawn on `/dev/tty`. Quitting the pager (`q`) kills the command
  and returns to the TUI.
- **Lessfilter**: The `preview`, `display` and `extract` presets render a compatible files' content into the preview pane.
- **`fs :tool pager`**: Behaves similarly to standard pagers, but provides syntax highlighting through bat, and reads options through environment variables.

## Configuration — `[pager]` section

Top-level TOML table in `config.toml`:

```toml
[pager]
bat_opts = ["--color=always", "--style=changes"]
line_numbers = false
follow = false
horizontal_scroll = false
prompt = "" # disable prompt text
```

| Field               | Type            | Default                                 | Meaning                                                       |
| ------------------- | --------------- | --------------------------------------- | ------------------------------------------------------------- |
| `bat_opts`          | list of strings | `["--color=always", "--style=changes"]` | Extra arguments passed to `bat`.                              |
| `line_numbers`      | bool            | `false`                                 | Show line numbers in the pager.                               |
| `follow`            | bool            | `false`                                 | Auto-scroll as new output arrives (follow mode).              |
| `horizontal_scroll` | bool            | `false`                                 | Always enable horizontal scrolling (Ctrl+h still toggles it). |
| `prompt`            | string          | `"/ or ? to search"`                    | Footer prompt text shown by the pager.                        |

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

Without `PG_FLAGS` or `PG_RAW`, bat runs with
`--color=always --paging=never --style=changes` by default.

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

- `src/pager.rs` — paging core (`page`, `page_reader`, `render_text`).
- `src/lessfilter/helpers.rs` — `env_bat_opts()`: environment → bat arguments.
- `src/run/register/execute.rs` — paged execution plumbing.
