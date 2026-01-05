Too long paths... a metadata preview mode?

# Actions

Platform specific feature flags for dependencies

# Panes

Nav: - search depth
Custom: - sorting?
pane settings need to be applied not just on startup
When entering non-nav, cursor should enter prompt

# Execute
Enter a terminal (works), and return to fs on ctrl-c (asking a bit much?)
mini terminal?
lowpri: Preview command also needs to switch cwd if there is an efficient way

# Indexing

- (alt)0-9 to accept? not sure what a good way to set this up would be.

# Trash is unreliable!

# Lessfilter

Script

- Previewer Formats

  - chafa
  - format
  - pdf/epub
  - sqlite

- Text rendering
  - Currently we always just use bat
  - rle_matcher is not perfect because it doesn't support expressing fallback conditions
  - prettier + tmp file formatting isn't hard but maybe out of scope?

Image rendering

- lowpri
- ratatui-image doesn't seem very reliable, we may have to do it manually, maybe check out yazi
- scaling

- Verbose Previewer
- Terminal Opener

we should have some functions, which users can pair?

# Db

- option to register on action
- Configurable prompts for each pane

# Apps

# Other

- Presets
- Debounce rerenders to avoid showing 0 items

# Scratch

# Global Filters

- we shouldn't conflate DbSort and SortOrder: todo: seperate them and support them in filtersoverlay

- Better column sizing, display paths from end
- Table editing

# FD

changing the visibility should invalidate find
maybe when given paths which don't exist on cli, find should treat the query as on absolute paths somehow.

- More filters (mtime or sth)
- Syncopated sorting?
- Wrap the default command sort/visibility fields in option with optional struct, add config-powered default values and merge from those.

# Menu overlay

Custom actions

- Compress/extract
- Copy:
  - real path
  - all properties
  - merge into
  - show diff
- backup

Per-pane extensions:

## App

// reveal
// open items selected by finder
// copy bundle id

// linux: editable
// reveal

# Saved files

- menu bar of files/folders: allow typing out exact path, with underline when valid and completion
- bookmarks?

# Important

- lessfilter

# For AI

Refactor stack
Add operations support on prompt

# Low pri

- watcher can potentially lose events due to sleep. But we only want a single dispatch for each simultaneous event. Is there an edge-trigger mode?

error on empty?

# Scripts

we have some zsh specific scripts
