# Architecture

## pager

The built-in pager (`src/pager.rs`) renders uncolored streams and fallback text
through interactive [minus](https://docs.rs/minus/latest/minus/).

### Entry points

- `page_child(child)` — pages a spawned child's stdout to `/dev/tty` (from
  `cba::broc::TTY_HANDLE`) for `ExecPaged`/`LFPaged` so the TUI keeps
  interacting with the child while its output pages on the tty.
- `page_reader(r, force_tty)` — pages any reader (`force_tty = true` for
  `MenuActionExecPaged`, `Help`, and `OverlayHelp`; `force_tty = false` for
  `lessfilter::extract`).
- `page_file(path)` — uncolored fallback used by `lessfilter` `CommandStrategy::Pager`
  when `mm` is absent from `PATH`; pages interactively on a TTY stdout or copies
  to stdout when piped.

File/markdown/directory/syntax-highlighted paging in `lessfilter` delegates to
`PG_IMAGE=false mm pager` when `mm` is available.

### Global DB

DB is a OnceLock: this is fine as long as tests don't depend on it. In particular, `db/crud.rs` creates its own Pool.
