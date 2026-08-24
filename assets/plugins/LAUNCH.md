---
name: fist-launch
description: Launches F:ist panes from the command line with the right subcommand, visibility filters, sort order, and pane options. Use when scripting or keybinding fs invocations - find/search/custom-stream/history/app panes, choosing -h/-a/-F style filter flags, passing fd or rg arguments, setting initial queries, or printing results with --format/--output-sep.
compatibility: Requires the F:ist CLI (`fs`); the find and search panes additionally require fd-find and ripgrep on PATH. Flag surface reflects fs's clap definition; run `fs <subcommand> --help` to confirm against the installed version.
---

# Launching f:ist panes from the CLI

Use this skill to construct `fs` invocations that open exactly the desired pane with the desired filters, sort, and options. The `--list` non-interactive variants are out of scope for this skill.

## Source of truth

```sh
fs --help                 # top-level flags and subcommands
fs ::  --help             # find (default) — also what bare `fs [pattern]` uses
fs :   --help             # search (:rg)
fs :custom --help         # custom stream pane (:c)
fs :dir --help            # recent folders
fs :file --help           # recent files
fs :o    --help           # app launcher (:open)
```

## Subcommand map

| Invocation                                         | Pane            | Behavior                                                                           |
| -------------------------------------------------- | --------------- | ---------------------------------------------------------------------------------- |
| `fs`                                               | Nav             | Browse from the current directory                                                  |
| `fs [PATHS]... [PATTERN] [-- FD_ARGS]`, alias `::` | Find            | Recursive fd-backed search; **last positional is the pattern**, the rest are paths |
| `fs : [PATTERNS]... [-- RG_ARGS]`, alias `:rg`     | Search          | Full-text search; multiple patterns are OR'd; quoted patterns keep whitespace      |
| `fs :custom [CMD]...`, alias `:c`                  | Custom          | Runs CMD and lists its stdout lines; empty CMD reads stdin                         |
| `fs :dir [--sort S] [QUERY]...`                    | Folders history | frecency-sorted directory jump pane                                                |
| `fs :file [--sort S] [--query Q]`                  | Files history   | frecency-sorted file browser                                                       |
| `fs :o [-w PROG] [FILES]...`, alias `:open`        | Apps            | Pick an application to open FILES with                                             |
| `fs :info`                                         | —               | Print database/history statistics (non-interactive)                                |
| `fs :tool ...`                                     | —               | Secondary tools (`lessfilter`, `pager`, `liza`, `trash`, ...)                      |

The default find subcommand doubles as a zoxide-like printer with `--cd`: it prints the first match instead of opening the TUI, resolving multiple keywords against the history database.

## Global options

Apply to any pane-launching invocation:

- `-v` / `-q`: verbosity counters (default level 4; `FS_VERBOSITY` env otherwise).
- `--config PATH`, `--override PATH` (config override), `--mm-config PATH`: configuration sources.
- `--style <auto|icons|icon-colors|colors|none|all>`: row styling override.
- `--fullscreen[=true|false]`: fullscreen mode and (with a value) result ordering direction.
- `--lock-prompt <bool>`: override prompt-locking behavior for this run.
- `--alt-accept`: swap Enter/Alt-Enter semantics (print instead of open).
- `--format TEMPLATE`, `--output-sep SEP`, `--opener PROG`: control printed results and the accept/open program.

## Visibility filters

Shared by the find, search, and custom panes (each flag takes an optional explicit bool):

| Flag                  | Effect                                  |
| --------------------- | --------------------------------------- |
| `-h[=B]`              | include hidden entries                  |
| `-I[=B]`              | hide ignored entries                    |
| `-a[=B]` (alias `-u`) | show all (disables existence filtering) |
| `-F[=B]`              | only directories                        |
| `-f[=B]`              | only files                              |

Defaults: inside a git repository hidden+ignored are handled automatically; outside, everything visible. A find pattern starting with `.` auto-enables hidden entries. `-F`/`-f` are mutually exclusive; the last one wins. `--reset-visibility` (find) ignores configured per-pane defaults.

## Sort orders

`--sort <name|mtime|atime|size|none>`. Validity is per pane:

- Find/Nav/Custom/Stash: all five, applied as a soft sort over query relevance.
- Search: no `size` (rg cannot size-sort); an invalid value falls back to `none`.
- History panes (`:dir`, `:file`, apps): SQL-backed labels — `none`=frecency, `size`=entry count, `mtime`=insertion order.

## The find pane (`::` / bare `fs [pattern]`)

```sh
fs                          # nav pane
fs notes                    # find 'notes' from cwd (or $HOME per config)
fs ~/src ~/docs TODO        # paths first, pattern last
fs -t rs -t .md -F src      # extensions/categories + dirs-only
fs -- --changed-within 1d   # raw fd args after --
fs --cd proj                # print best match (zoxide-style), no TUI
```

- `-t/--types` accepts comma-separated values, repeatable: file types (`d`, `l`, `x`, ...), categories (`image/i`, `video/v`, ...), extensions (`.rs`; empty string = no extension), and custom groups defined in lessfilter `[categories]`. `fs :tool types` prints all mappings. Asking only for directories toggles dirs-only visibility automatically (and vice versa).
- `-A/--no-all` neutralizes the `all` visibility bit.
- `--transform LUA|@file.lua` rewrites each row before display (see the custom pane contract below).

## The search pane (`:` / `:rg`)

```sh
fs : TODO                   # regex search for TODO under cwd
fs : -p ~/proj fix '--iglob !vendor'   # scoped path + raw rg passthrough
fs : -i -C 2 'panic!'       # case-insensitive, context lines
fs : --fixed-strings '$var' # literal match
fs : -1 error               # one line per match (no grouping)
```

Key flags: `-p/--path` (repeatable), positional PATTERNS (regex by default), case group `-i/-s/-S`, context group `-A/-B/-C NUM`, `--one-line`, `--fixed-strings` / `--no-fixed-strings`, `--preserve-whitespace` (treat the query literally), `--rebase` (run from the deepest common directory), `--filtering` (start in filter mode rather than query mode), `--query STR` (initial input), `--no-read` (ignore stdin). Everything after `--` goes to rg verbatim.

The pane has two modes sharing one input: *query* mode repopulates results from rg as you type; *filter* mode narrows existing rows (prefix with `%` to filter the second/context column). Opening a match stores `HIGHLIGHT_LINE`/`HIGHLIGHT_COLUMN` in the environment so editors can jump to the exact position.

## The custom stream pane (`:custom` / `:c`)

```sh
git status --porcelain | fs :custom          # read stdin
fs :custom git status --short                # run a command
fd -0 -tf | fs :c --input-sep '\0'
fs :c --tail-sep $'\t' --transform @map.lua -- generator
```

- `--tail-sep CHAR` splits each line into `(path, tail)`; the tail renders in the context column.
- `--input-sep CHAR` splits records on something other than newline (e.g. `\0`).
- `--sort`, visibility flags, `--opener`, `--cd`, and `--transform` behave as elsewhere.
- `--transform` receives `(path, tail)` and returns up to three values `(path, display, tail)`: returning nil for `path` drops the row, omitting display/tail keeps current values. Inline Lua or `@file.lua`.

## Choosing the right invocation

1. Directory browsing → bare `fs`; jump-by-memory → `z` shell function or `fs :dir`.
2. Known name/glob/type across a tree → find pane with `-t` filters; pass exotic fd options after `--`.
3. Searching contents → search pane; quote multi-word patterns; use `--fixed-strings` for literals.
4. External list (build artifacts, notes index, git output) → pipe into `:custom`, optionally with `--tail-sep` and a transform.
5. "Open this with what?" → `fs :o -w` or feed files via `fs :o FILE...`.
6. Scripted consumption → prefer `--cd` (find/dirs) which prints and exits, plus `--format`/`--output-sep` when integrating printed selections.

## Common failure modes

- **Pattern treated as a path:** the find pane takes PATHS first and PATTERN last — reorder arguments.
- **fd/rg flags swallowed by fs:** they must come after `--`.
- **`size` sort silently ignored on the search pane:** unsupported there; pick another order.
- **Hidden files missing unexpectedly:** a git-repo default or a stale `-h=false` is in effect; pass `-h` explicitly or `--reset-visibility`.
- **Results polluted by a piped path list:** the search pane reads stdin by default when piped; pass `--no-read` to ignore it.
- **Quoted pattern split into several:** single-quote it (`'foo bar'`), or start with `'` via `--preserve-whitespace`.
