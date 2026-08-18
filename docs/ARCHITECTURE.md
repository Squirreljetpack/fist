# Architecture

## pager

The pager (`src/pager.rs`) renders a stream or file through interactive
[minus](https://crossterm.dev/) with optional `bat` syntax highlighting.

### Entry points

Three public functions, each taking `bat: Option<Vec<String>>` (bat
passthrough args); all funnel into `page_inner`:

- `page(child, force_tty, bat)` — pages a spawned child's stdout (the
  `ExecPaged`/`LfPaged` execute paths). `force_tty=true` drives minus's output
  sink to `/dev/tty` (from `cba::broc::TTY_HANDLE`), so the TUI keeps
  interacting with the child while its output pages on the tty.
- `page_reader(r, force_tty, bat)` — pages any reader (no child); used by the
  paged-lua flow (`force_tty=true`) and by subtool tools (`force_tty=false`).
- `render_text(path, bat)` — renders a file to the current process's stdout
  (subtool path; the file goes to bat directly so language detection works).

`force_tty=false` (subtool paths) probes stdout: a terminal gets interactive
minus, a pipe or file is passed straight through bat-colored, no paging UI.

`configure_pager(pager)` is the only place the module reads config: it loads
`pager_cfg()` from `pager.toml` for the minus knobs (`line_numbers`, `follow`,
`horizontal_scroll`, `prompt`, `smart_case`). It never sources `bat_opts`
itself — callers do.

### Where bat opts come from

Two sides converge on `pager_cfg().bat_opts` (`pager.toml`) as the base:

- **Parent (Paged execution):** `STORE::get_bat_opts()` (in `run/state/temp.rs`)
  merges `pager_cfg().bat_opts` with one-shot `STORE` extras
  (`set_bat_opts`, used by the lessfilter help flow). Decided once in the
  execute handler; when bat runs, the child gets `PG_RAW=true` so the spawned
  subtool skips its own bat rendering.
- **Subtool (lessfilter / `fs :tool pager`):** `env_bat_opts()`
  (`lessfilter/helpers.rs`) resolves env overrides (`PG_FLAGS`, `PG_LANG`,
  `PG_FILENAME`, `HIGHLIGHT_LINE`) on top of the `pager_cfg().bat_opts`
  default; `None` (from `PG_RAW=true` or `bat_opts = None`) skips bat.

`bat = None` everywhere means raw passthrough with no highlighting; `Some`
pipes the stream through the `bat` binary when it is on `PATH`, spawning it
with the given args.
