# Actions

Platform specific feature flags for dependencies

# Docs

- Intro guide
- FS_OPTS

# Panes

Nav: - search depth
Custom: - sorting?

# Footer

- empty directory

# Lessfilter (lowpri)

- Previewer Formats
  - chafa
  - format
  - pdf/epub
  - sqlite

- configurable extractor

- Low pri
  - rle_matcher is not perfect because it doesn't support expressing fallback conditions
  - prettier + tmp file formatting isn't hard but maybe out of scope?

Image rendering

- lowpri
- ratatui-image doesn't seem very reliable, we may have to do it manually, maybe check out yazi
- scaling
- note: since we plan to require reading images anyway, baking in the clipboard instead of relying on an external program makes sense.

Better support for some of the modes

# Archives

transparent advance into compressed files

- compress and unzip actions
- transparent preview into compressed files

# Other

- Theme?
- Debounce rerenders to avoid showing 0 items

# Rg

# Global Filters

- we shouldn't conflate DbSort and SortOrder: todo: seperate them and support them in filtersoverlay

- Better column sizing, display paths from end
- Table editing

# Disk

- Size tree

# FD

- More filters (mtime or sth)
- Syncopated sorting?
- Cli structs should be partial to merge into visibility?

# Menu overlay

Custom actions

- Compress/extract
- Copy:
  - real path
  - all properties
  - merge into
  - show diff
- backup
- replace_text
  - opt: regex
  - opt: multi
  - opt: preserve_case
- git status, lz ui

Possible Conditions:

- Selection #
- Pane
- Condition on selected file(s)
- shell command
- various other mm state

## App

// reveal
// open items selected by finder
// copy bundle id

// linux: editable
// reveal

# Saved file pane ideas

- menu bar of files/folders: allow typing out exact path, with underline when valid and completion
- bookmarks?
- version controlled, generic, incremental backups

# Low pri

- undo redo on file actions (copy/etc.)
- Built-in edit under a feature flag? https://github.com/microsoft/edit/tree/main
- https://github.com/unicode-org/icu4x : for table printing
- Contained mini terminal would be cool
- finer control of bumping from actions (Lessfilter/Execute/Become/PrintAccept)
- configurable prompt styling
- fstoggle: reverse

# Stash

- overridden actions should hide the overridden toast as well somehow.
- persist?

# Bugs

- sync handler never runs if no elements
  - need display toast on empty dir
- how to make file operations feel bulletproof

# Shell

- currently completions seems to complete flags, why doesn't it complete subcommands?
- we have some zsh specific scripts, posix/something crossplatform would be better.
- aging algorithm
- prepopulate some directories: trash, desktop, home etc.
- option for second pass on z jump fail: search children and parents

# Per pane mm/ui configs:

- in particular: reverse/colors
- enable partial?
- Need figure out how defaults should be specified: in code or in config.
- PaneSettings: prompt and show_preview are effectively partial, but not sure if we should just generalize that to supporting partial RenderConfig overrides. The size is bounded so it shouldn't be too bad?
- Also, note that undo/redo won't save state changes caused by pane change overrides
