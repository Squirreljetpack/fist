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
