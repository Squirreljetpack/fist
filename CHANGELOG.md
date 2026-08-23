## [0.0.5] - 2026-08-23

### 🚀 Features

- Shell init support for shells besides zsh
- Improve queue progress bar visuals
- Vendor scripts into rust code
- Copy/move worker v1

### 🐛 Bug Fixes

- Various

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
