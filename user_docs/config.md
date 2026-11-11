# config.toml

`config.toml` configures the application itself: pane behavior, history,
transfers, the filesystem watcher, and path styling. UI configuration lives separately in [mm.toml](mm.md); the pager, lessfilter, and
menu actions have their own files as well.

| File                       | Purpose                                                                       |
| -------------------------- | ----------------------------------------------------------------------------- |
| `config.toml`              | This page.                                                                    |
| `mm.toml`                  | Columns, previews, TUI layout, overlays, key bindings — see [mm.toml](mm.md). |
| `lessfilter.toml`          | Preview and handler presets.                                                  |
| `pager.toml`               | The pager — see [The Pager](pager.md).                                        |
| `actions.toml`, `actions/` | Menu actions — see [Menu Actions](menu-actions.md).                           |

The config directory is the platform config directory (`~/.config/fist` on
Linux). An existing `~/.config/fist` is preferred on every platform, even
when `XDG_CONFIG_HOME` points elsewhere.
`fs --dump-config` writes the shipped defaults for all config files when
stdout is a terminal, skipping files that already exist, and prints the fully
resolved `config.toml` when piped.

Values omitted from the file keep their defaults. Unknown keys are rejected
with an error.

## Top level

| Key         | Type | Default               | Meaning                                     |
| ----------- | ---- | --------------------- | ------------------------------------------- |
| `state_dir` | path | `~/.local/state/fist` | Database, logs, and other persistent state. |
| `cache_dir` | path | `~/.cache/fist`       | Cache data.                                 |

On macOS and Windows these follow the platform state and cache directories.

## Panes

`[panes.<name>]` configures the behavior of one pane. The common fields are:

| Key                    | Type            | Default       | Meaning                                                                                                                                                                                                    |
| ---------------------- | --------------- | ------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `prompt`               | string          | none          | Query-bar prompt.                                                                                                                                                                                          |
| `prompt_style`         | style           | none          | Style of the prompt.                                                                                                                                                                                       |
| `show_preview`         | bool or integer | pane-specific | Show the preview when entering the pane. A number is the minimum terminal width — or height for top/bottom previews — required to show it. A pane without a value inherits the current preview visibility. |
| `lock_prompt`          | `when`          | `"never"`     | Enter the query bar on pane entry.                                                                                                                                                                         |
| `preview_layout_index` | integer         | `0`           | Index into the `[[preview.layout]]` list in [mm.toml](mm.md).                                                                                                                                              |

### Nav

`[panes.nav]` adds:

| Key                  | Type           | Default   | Meaning                                                                                                                                                      |
| -------------------- | -------------- | --------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `default_sort`       | sort order     | `"mtime"` | Initial sort.                                                                                                                                                |
| `default_visibility` | visibility     | none      | Initial hidden/ignored/file filters. When omitted, hidden files are shown and ignored files are hidden inside a git repository, and the reverse outside one. |
| `ignore_patterns`    | array of globs | `[]`      | Gitignore-style patterns the directory walker skips. Matched against paths relative to the listed directory. Ignored while the `all` visibility is engaged.  |

### Find

`[panes.find]` adds:

| Key                                    | Type             | Default           | Meaning                                                                                         |
| -------------------------------------- | ---------------- | ----------------- | ----------------------------------------------------------------------------------------------- |
| `default_visibility`                   | visibility       | none              | Initial visibility; see nav.                                                                    |
| `default_sort`                         | sort order       | none              | Initial sort.                                                                                   |
| `max_refresh_items_threshold`          | integer          | `20000`           | Enable the directory watcher to auto-refresh results when fewer than this many items are added. |
| `max_refresh_execution_time_threshold` | milliseconds     | `400`             | Also enable the watcher when the search finishes within this time.                              |
| `reduce_paths`                         | bool             | `false`           | Change the working directory to the common parent of the paths given to `fs`.                   |
| `exclusions`                           | array of globs   | platform-specific | Paths excluded from every `fd` search. `~` expands to your home directory.                      |
| `base_args`                            | array of strings | `[]`              | Arguments added to every `fd` command.                                                          |
| `default_args`                         | array of strings | `[]`              | Arguments applied to `fs ::` when no `fd` arguments were given on the command line.             |

### Search

`[panes.search]` adds:

| Key                   | Type             | Default   | Meaning                                                                                                    |
| --------------------- | ---------------- | --------- | ---------------------------------------------------------------------------------------------------------- |
| `default_visibility`  | visibility       | none      | Initial visibility; see nav.                                                                               |
| `default_sort`        | sort order       | none      | Initial sort.                                                                                              |
| `one_line`            | bool             | `false`   | Put each match on one line instead of showing a context block.                                             |
| `fixed_strings`       | bool             | `false`   | Treat queries as fixed strings by default instead of regexes.                                              |
| `preserve_whitespace` | bool             | `false`   | Keep the query as one pattern instead of splitting it on whitespace.                                       |
| `rg_status_template`  | display template | none      | Matchmaker status-line template shown while searching with ripgrep.                                        |
| `fs_status_template`  | display template | none      | Matchmaker status-line template shown while filtering with f:st.                                           |
| `iglobs`              | table            | `{}`      | Per-directory ripgrep globs. The key is a folder; `""` applies everywhere. Values are passed as `--iglob`. |
| `base_args`           | array of strings | see below | Arguments added to every `rg` command.                                                                     |
| `empty_pattern`       | string           | none      | Pattern used when no query is given. A leading `-v` adds the `-v` flag.                                    |
| `default_args`        | array of strings | `[]`      | Arguments applied to `fs :` when no `rg` arguments were given on the command line.                         |

The default `base_args` are `--trim --color=ansi --no-context-separator
--field-context-separator=-`.

### Other panes

`[panes.app]`, `[panes.files]`, `[panes.dirs]`, and `[panes.custom]` share the
common fields. The first three add:

| Table   | Key                    | Type           | Default         | Meaning                                            |
| ------- | ---------------------- | -------------- | --------------- | -------------------------------------------------- |
| `app`   | `app_scan_directories` | array of paths | `[]`            | Extra directories to scan for applications.        |
| `app`   | `require_icon`         | bool           | `true` on macOS | Only list applications whose icon can be resolved. |
| `files` | `default_sort`         | sort order     | none            | Initial sort.                                      |
| `dirs`  | `default_sort`         | sort order     | none            | Initial sort.                                      |

### Stashes

Every stash is a pane addressed by name: `[panes.stashes.<name>]`, with
`[panes.stashes.""]` for the unnamed stash. Stashes share the common pane
fields and add:

| Key      | Type   | Default       | Meaning                                                       |
| -------- | ------ | ------------- | ------------------------------------------------------------- |
| `kind`   | string | `"transient"` | Where the stash keeps its entries; see below.                 |
| `insert` | string | `"replace"`   | What stashing a path that is already stashed does; see below. |

`kind` picks the storage:

- `"transient"` — in-memory. The stash starts empty on every run and never
  touches the database.
- `"filter"` — database-backed, hiding entries whose path no longer exists.
- `"prune"` — database-backed, also deleting those entries from the database
  while the pane is populated.

`insert` picks what adding an already-stashed path does:

- `"replace"` — drop the existing entry and add a fresh one, moving the path
  to the end of the stash with a new add time.
- `"skip"` — keep the existing entry only.
- `"duplicate"` — add another entry even though the path is already stashed.

A stash table without a `prompt` shows the stash name followed by `>`.
A stash with no table of its own falls back to a white prompt.

## Interface

`[interface]` controls the main interactions:

| Key                                 | Type                               | Default                | Meaning                                                                                                                                                     |
| ----------------------------------- | ---------------------------------- | ---------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `advance_command`                   | output template                    | lessfilter edit preset | Command run when `Advance` targets a file.                                                                                                                  |
| `alt_accept`                        | bool                               | `false`                | Swap the accept and print flavors of the accept keys.                                                                                                       |
| `no_multi_print`                    | bool                               | `false`                | Print only the current item, not the whole selection.                                                                                                       |
| `always_paste`                      | bool                               | `false`                | Treat paste as text entry even outside the query bar.                                                                                                       |
| `remove_db_record_action`           | `"None"`, `"Trash"`, or `"Delete"` | `"None"`               | Which action removes database records in history and stash panes. The other action operates on the real path. With `"None"`, both operate on the real path. |
| `hide_preview_when_cursor_disabled` | bool                               | `false`                | Hide the preview while the query bar is active.                                                                                                             |
| `allow_enter_unzip_directory`       | bool                               | `false`                | When false, `Parent` inside a mounted archive leaves the archive for the archive's own directory. When true, it walks up the extraction root instead.       |
| `stability_threshold`               | integer                            | `30`                   | How tolerant sorted results are to score changes between reloads. `0` always re-sorts.                                                                      |
| `preserve_size_cache`               | bool                               | `false`                | Keep the directory-size cache when changing to a directory that is already cached.                                                                          |
| `cwd_prompt`                        | output template                    | `"{=} "`               | Prompt shown while the query bar targets the current directory.                                                                                             |
| `toast_on_empty`                    | bool                               | `true`                 | Show a toast when the current directory is empty.                                                                                                           |
| `jump_or_advance`                   | bool                               | `false`                | Make `JumpOrAccept` advance instead of accept when the cursor is already on the target. `JumpOrAccept(0)` always accepts.                                   |
| `refill_selections_after_reload`    | bool                               | `true`                 | Restore selections after results are repopulated.                                                                                                           |
| `copy_files_to_clipboard`           | bool                               | `true`                 | Put the selected files on the system clipboard on `Copy`.                                                                                                   |
| `move_files_to_clipboard`           | bool                               | `false`                | Cut the selected files to the system clipboard on `Move`.                                                                                                   |
| `bump_menu_action_paths`            | bool                               | `false`                | Record menu-action targets in history, including actions that only enqueue them.                                                                            |
| `bump_transfer_destinations`        | bool                               | `false`                | Record successful transfer destinations in history.                                                                                                         |

## Queue

`[queue]` configures background transfers:

| Key                        | Type | Default | Meaning                                                                                      |
| -------------------------- | ---- | ------- | -------------------------------------------------------------------------------------------- |
| `clear_completed_on_close` | bool | `false` | When false, completed rows are cleared when the queue overlay closes. Set true to keep them. |

`[queue.copy]` and `[queue.move]` accept the same transfer settings:

| Key                 | Type                  | Default        | Meaning                                                                                                                                                                         |
| ------------------- | --------------------- | -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `workers`           | integer               | number of CPUs | Parallel transfer workers.                                                                                                                                                      |
| `preserve_metadata` | bool                  | `true`         | Preserve timestamps, permissions, and other metadata.                                                                                                                           |
| `reflink`           | `"Auto"` or `"Never"` | `"Auto"`       | Copy-on-write cloning when the filesystem supports it.                                                                                                                          |
| `conflict`          | string                | `"Overwrite"`  | What to do when an entry inside the destination already exists: `"Fail"`, `"Overwrite"`, `"Skip"`, `"RenameSuffix"`, or `"Abort"`.                                              |
| `root`              | string                | `"Rename"`     | What to do when the destination root already exists: `"Overwrite"`, `"Merge"`, `"Rename"`, or `"Fail"`. `Merge` resolves entries inside the existing directory with `conflict`. |
| `buffer_size`       | integer               | `524288`       | Copy buffer size in bytes.                                                                                                                                                      |

## Smart visibility

`[smart_visibility]` controls the automatic visibility heuristics:

| Key                       | Type   | Default  | Meaning                                                                                                                                                  |
| ------------------------- | ------ | -------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `dot_query_show_hidden`   | bool   | `true`   | Show hidden files when a find query starts with `.`.                                                                                                     |
| `dot_query_show_ignored`  | bool   | `false`  | Also show ignored files for dot-queries.                                                                                                                 |
| `empty_folders`           | bool   | `true`   | Show hidden files in a nav pane whose directory contains only hidden entries.                                                                            |
| `git_repo`                | `when` | `"auto"` | Enable git-aware visibility. `"auto"` applies it on entry; `"always"` also adapts when crossing repository boundaries at runtime; `"never"` disables it. |
| `ignored_targets`         | bool   | `true`   | Stop hiding ignored files when an explicitly given path is ignored by git.                                                                               |
| `retain_filetype_filters` | bool   | `false`  | Keep the file/directory-only filters when entering a nav pane.                                                                                           |

## History

`[history]` configures the visit database:

| Key                | Type           | Default       | Meaning                                                                                                                                               |
| ------------------ | -------------- | ------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| `resolve_symlinks` | bool           | `false`       | Resolve symlinks when checking whether a path still matches its recorded type.                                                                        |
| `query_strategy`   | string         | `"Substring"` | How queries match paths: `"Substring"` or `"Monotonic"`.                                                                                              |
| `case_sensitive`   | bool           | smart         | Force case-sensitive matching. When omitted, a query with uppercase characters is case-sensitive and one without is case-insensitive.                 |
| `lambda`           | float          | `0.008`       | Decay rate for frecency scoring. Each tick multiplies the stored score by `exp(-lambda * delta)`.                                                     |
| `exclude`          | array of globs | `[]`          | Paths that are never recorded or bumped. `~` expands to your home directory.                                                                          |
| `prune_max`        | integer        | `10000`       | Prune the database once it exceeds this many entries.                                                                                                 |
| `prune_min`        | integer        | `8000`        | Number of entries kept after pruning.                                                                                                                 |
| `refind`           | string         | `"Next"`      | What to do when the best match is the current directory: `"Search"` opens the interactive search, `"Next"` keeps looking, `"None"` returns it anyway. |
| `show_missing`     | bool           | `false`       | Include entries whose paths no longer exist.                                                                                                          |
| `base_dir`         | path           | none          | Only record paths below this directory.                                                                                                               |

## Watcher

`[notify]` configures filesystem watching:

| Key                                | Type         | Default | Meaning                                                                    |
| ---------------------------------- | ------------ | ------- | -------------------------------------------------------------------------- |
| `fs_poll_ms`                       | milliseconds | `2000`  | Polling interval.                                                          |
| `debounce_ms`                      | milliseconds | `100`   | Ignore events that arrive within this interval.                            |
| `thrash_threshold.count`           | integer      | `5`     | Number of events within `duration_ms` that trips the event-storm throttle. |
| `thrash_threshold.duration_ms`     | milliseconds | `5000`  | Sliding window for that count.                                             |
| `thrash_threshold.resume_delay_ms` | milliseconds | `10000` | Quiet period after the last event before updates resume.                   |

## Miscellaneous

`[misc]` contains tool and process behavior:

| Key                         | Type             | Default | Meaning                                                                                                    |
| --------------------------- | ---------------- | ------- | ---------------------------------------------------------------------------------------------------------- |
| `append_mode_logging`       | bool             | `false` | Append to the application log instead of overwriting it at startup.                                        |
| `tools_append_mode_logging` | bool             | `false` | Same, for the tool log.                                                                                    |
| `spawn_with`                | array of strings | `[]`    | Prefix used when spawning programs, for example a process manager: `["pueue", "add", "-g", "apps", "--"]`. |
| `list_absolute_paths`       | bool             | `false` | Print absolute paths in `--list` output.                                                                   |

## Styles

`[styles.path]` configures how paths are rendered:

| Key             | Type | Default | Meaning                                          |
| --------------- | ---- | ------- | ------------------------------------------------ |
| `collapse_home` | bool | `true`  | Shorten the home directory to `~`.               |
| `relative`      | bool | `true`  | Display paths relative to the current directory. |
| `file_icons`    | bool | `true`  | Show file icons.                                 |
| `file_colors`   | bool | `true`  | Color files.                                     |
| `dir_icons`     | bool | `true`  | Show directory icons.                            |
| `dir_colors`    | bool | `true`  | Color directories.                               |
| `icon_colors`   | bool | `true`  | Color icons.                                     |

`[styles.path.file_styles]` overrides the style for each file category:
`image`, `video`, `music`, `lossless`, `crypto`, `document`, `compressed`,
`temp`, `compiled`, `build`, `source`, `configuration`, `text`, and `other`.
These entries are ratatui styles with `fg`, `bg`, `add_modifier`,
`sub_modifier`, and `underline_color`.

Other style sections:

| Table                       | Meaning                                                                                       |
| --------------------------- | --------------------------------------------------------------------------------------------- |
| `[styles.matchmaker]`       | `horizontal_separator`: `"None"`, `"Empty"`, `"Light"`, `"Normal"`, `"Heavy"`, or `"Dashed"`. |
| `[styles.toast]`            | One style per toast level: `normal`, `info`, `success`, `warning`, `error`.                   |
| `[styles.raw_prompt_style]` | Style of the raw query text.                                                                  |

`--style` overrides `file_icons`, `file_colors`, `dir_icons`, `dir_colors`,
and `icon_colors` for one run.

## Value formats

- **`when`** — `"never"`, `"auto"`, or `"always"`.
- **sort order** — `"name"`, `"mtime"`, `"atime"`, `"size"`, or `"none"`.
  `"frecency"` is accepted as an alias for `"none"`, which is the frecency
  ordering in the database-backed panes (`files`, `dirs`, and `apps`).
- **visibility** — a table with any of `hidden`, `ignore`, `unrestricted`,
  `files`, and `follow`. Booleans are explicit; omitted keys inherit.
  `files = true` shows files only; `files = false` shows directories only.
- **style** — `fg`, `bg`, and `modifier` for `StyleSetting` tables. Colors
  are named colors or hex values; modifiers are pipe-separated, for example
  `modifier = "BOLD | ITALIC"`.
- **output template** — placeholders expanded against the current item; see [Templates](../docs/TEMPLATING.md). Used by `advance_command` and `cwd_prompt`.
- **display template** — matchmaker's status-line template syntax, see the [matchmaker configuration docs](https://squirreljetpack.github.io/matchmaker-docs/templating).
- **milliseconds** — integer milliseconds, for example `400`.
