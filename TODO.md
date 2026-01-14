# Actions

Platform specific feature flags for dependencies

# Docs

- Intro guide
- FS_OPTS

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

- (alt)0-9 to accept? Feature is in but not sure what is a good default/behavior for this.

# Lessfilter (lowpri)

- Previewer Formats

  - chafa
  - format
  - pdf/epub
  - sqlite

- Low pri
  - rle_matcher is not perfect because it doesn't support expressing fallback conditions
  - prettier + tmp file formatting isn't hard but maybe out of scope?

Image rendering

- lowpri
- ratatui-image doesn't seem very reliable, we may have to do it manually, maybe check out yazi
- scaling

Better support for some of the modes

-

# Db

- option to register on action
- Configurable prompts for each pane (blocker: config is not mutable)

# Other

- Presets
- Debounce rerenders to avoid showing 0 items

# Rg

- how to display it comfortably?
- Allow configuring tokens from actions

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

- nav ignore?

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

# Low pri

- watcher can potentially lose events due to sleep. (i.e. trash) But we only want a single dispatch for each simultaneous event. Is there an edge-trigger mode?
- undo redo on file actions (copy/etc.)
  error on empty?
- async push for display_script
- pueue should be optional, especially since it doesn't handle single quotes correctly: pass cfg down to spawn or use a global?

# Scripts

we have some zsh specific scripts, posix/something crossplatform would be better.

# UI

...

# CLI

better cli/env var handling: need a proc macro to generate a mirrored with every concrete value wrapped in option, then a fn to merge that config into the main config

# CD

better cd: swapping accept and print is too error prone, we need a global var
maybe add children with score 0?
aging algorithm

# Stash

- overridden actions should hide the overridden toast as well somehow.

# Bugs

- sync handler never runs if no elements
- copy/cut cause lag
