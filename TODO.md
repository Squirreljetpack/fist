# Perf

with a lot of items, ui gets laggy, where is the blocking?
Arc str is probably faster for context: check this
Arc str for render storage?

( fs .; ) 63.56s user 12.48s system 359% cpu 21.143 total # to 350k

lessfilter preview: .desktop, .app, .exe: display icons

- review and test
  tools: trash

lessfilter chafa:

- pass &run to to_progs
- image_viewer take &image_viewer as override
- test sixel

# nnn

- Create, list, extract (to), mount (FUSE based) archives
- Stack browser
- remote mounts

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
sudo prompting (for this and other actions)

# Archives

transparent advance into compressed files

- compress and unzip actions
- transparent preview into compressed files

# Other

- Theme?
- Debounce rerenders to avoid showing 0 items

# Rg

- On non-nav panes, this should populate with the currently selected/stored files

# Global Filters

- we shouldn't conflate DbSort and SortOrder
  - todo: seperate them and support them in filtersoverlay

- Better column sizing, display paths from end
- Table editing

- better per-pane filters: fd/rg probably wants to toggle --follow (in own pane probably), not sure about anything else
- does visibility apply to history panes? if so, --all should disable filtering out the nonexistant entries

# Disk

- Spawn a thread to build a size tree data structure (mutex). On completion, send a message to reload to the pane. Nav pane checks if current directory is available in the size tree and mutex is unlocked, otherwise fires off another task and doesn't reload.
- When computing, we acquire a mutex lock to the shared size tree. We do the standard method of creating a size tree with par_iter. If the shared size tree total size > 100 GB and is a subpath of our currently searched path, in our loop we should take that precomputed bit instead of recomputing.

# FD

- More filters (mtime or sth)
- Syncopated sorting?
- Cli structs should be partial to merge into visibility?
- toggleable executable filter
- only UTF-8 paths are processed from command output (fd, rg, custom)

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
- a bit annoying that the default search of rg is on paths, it should be able to configure to search on both or the other
- icon only coloring
- basic support for multi-line filenames (fd, rg should use null delimiters)
- Check that "interactive-shell-mode" pager works on all shells, not just zsh
- panes:
  - Nav: - search depth
  - Custom: - sorting?
- package/build?
- https://github.com/moretension/duti

- custom pager binary combining bat, minus and our special requirements

# Stash

- overridden actions should hide the overridden toast as well somehow.
- persist on disk?
- how to make file operations feel bulletproof
-

# Shell

- currently completions seems to complete flags, why doesn't it complete subcommands?
- we have some zsh specific scripts, posix/something crossplatform would be better.
- aging algorithm
- prepopulate some directories: trash, desktop, home etc.
- option to auto-add children and parents of visited directories
- option for second pass on z jump fail: search children and parents

# Per pane mm/ui configs:

- in particular: reverse/colors
- enable partial?
- Need figure out how defaults should be specified: in code or in config.
- PaneSettings: prompt and show_preview are effectively partial, but not sure if we should just generalize that to supporting partial RenderConfig overrides. The size is bounded so it shouldn't be too bad?
- Also, note that undo/redo won't save state changes caused by pane change overrides
- maybe pane prompts should not be configurable

# Plugins

- Custom panes as wasm?
  - music/git
