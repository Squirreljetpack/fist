## [0.0.6] - 2026-11-20

### 🐛 Bug Fixes

- Compile on targets without a SIMD kernel (wasm), and keep the icon path off when no image feature is enabled
- *(ci)* Publish to npm on tag releases; a called workflow sees the caller's `push` event, so the old `workflow_call` gate skipped every publish

### 💼 Other

- *(dist)* Ship an `aarch64-unknown-linux-musl` build

## [0.0.5] - 2026-11-20

A broad release focused on configurability, navigation, and day-to-day polish.

- Menus, actions, and bindings are now configurable, with pane-scoped modes, multiselect batch actions, Lua action modules, and confirm/wait execution modes.
- The pager gained follow mode, goto, copy-on-select, and mouse scrolling, with better handling of piped input.
- Stashes, history, and jump behavior were reworked: configurable stash panes, history exclusions, the `Z` history jump, and `JumpOrAccept`.
- Shell integration now covers more shells, with safer init scripts and prompt redraws.
- Added mount v1, system clipboard copy, native lessfilter image previews, and richer find/search panes.
- Archive extraction is now contained against path and symlink escapes, and Windows file opening and quoting were fixed.
- Rendering, watchers, queueing, and the copy/move workers received performance and stability work.
- CI now covers Linux and macOS alongside Windows, and packaging metadata and install docs were refreshed.

## [0.0.4] - 2026-08-19

### 🚀 Features

- Alt-accept cli flag
- Robust cwd, jump rework, and fullscreen modes
- Smart autovisibility, autorefresh, and trash improvements
- Tools log, NewDir, and state restoration
- Keybinding improvements, zoxide scoring, and trash tool
- Stash database and stash panes
- Wildcard file rules
- Menu_action scaffolding
- Menu actions in the overlay and queue renames
- App overlay
- Live overlay state and paged lua execution
- Db-backed deletes and apps-pane improvements
- Unzip and log fixes
- Custom type groups and :tool ds
- Isolated lua VMs for menu actions
- Ds multi-input tree flags
- Rename cut to move
- Pager
- Docs
- Improve toasts
- Pager cb

### 🐛 Bug Fixes

- Paste panic and queue overlay deadlock
- Walker .git ignore and final fixes
- Various

### 🚜 Refactor

- Stash rework and mimalloc allocator
- PathItem and :custom stream redesign
- Stash state and dispatch by kind
- Apps pane and operation queue
- Per-stash settings and transient stashes

### 📚 Documentation

- Update README

## [0.0.3] - 2026-03-11

### 🚀 Features

- Adjust shell defaults
- Shell fixes + begin support shell completions
- Rework lessfilter metadata processing
- Add some hueristics to fd cmd builder (smart hidden and no_ext) for more convenient cli usage
- Use joinset to for safer shutdown with long-running tasks.
- Score weighting
- Zsh keybinds
- Display-batch + code cleanup
- Preview line highlight
- Support cli fullscreen flag
- Open_with menu action
- --style=icon-colors, --enter-prompt
- Resolve_symlinks and query_strategy options in HistoryConfig
- ConfirmOverlay

### 🐛 Bug Fixes

- Retained piping for FsAction::Display

### 💼 Other

- Update deps

### 🚜 Refactor

- Migrate from Effect to direct action handling
- Remove wrapper types in actions + fixes

### 🎨 Styling

- Lints

## [0.0.2] - 2026-01-14

- Support actions parsing
- make spawn_with configurable
- fix stash queue
- various fixes and improvements

## [0.0.1] - 2026-01-05

Initial release
