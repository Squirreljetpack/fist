# F:ist

F:ist is a fast and intuitive search tool for the filesystem.

// video

# Installation

```shell
# dependencies
cargo install bat fd-find eza ripgrep

cargo install fist

# (Optional) setup shell integration:
echo "\neval$(fs :tool shell)" >> ~/.zshrc # or whatever the startup file of your respective shell is.
```

Call as:

- `fs`: Directory navigation
- `fs [..paths] pattern`: interactive find
- `generate_paths | fs`: enriched fuzzy searching of paths
- `z [query]`: directory jump (requires [shell integration](#shell-integration))

# Commands

### (Default) bindings overview

- `Up`/`Down`: Navigate (or `Up` in the initial position to to enter prompt).
- `Left`/`Right`: Back/Enter.
- `Enter`: Default (system) open.

---

- `ctrl-f`/`ctrl-r`: Find files/Search text.
- `ctrl-g`: History view (Folders and files).
- `ctrl-z`/`ctrl-y`: Undo/Redo.

---

- `ctrl-x`/`ctrl-c`/`ctrl-v`: Cut, Copy, Paste.
- `delete/shift-delete`: Trash/Delete.
- `ctrl-e`: Open menu.
- `ctrl-s`: Open stash.
- `ctrl-shift-f`: Open filters.
- `ctrl-h`: Toggle hidden.

---

- `Tab`: Toggle select.
- `alt-enter`: Print.
- `?`: toggle preview
- `ctrl-b`: Open background.
- `ctrl-l`: Full preview.
- `/` and `~`: Jump to home

For a full list of binds within the app, type `ctrl-shift-h`[^3].\
For more information on bindings, see [matchmaker](https://github.com/Squirreljetpack/matchmaker).

[^3]: in the same order .

# Panes

F:ist records the files, directories and applications that you've visited in a local database, using it to sort the results in the `App` and `History` panes by relevance.

## Fd

## Rg

fs has two columns: the main filepath column, and sometimes a secondary context column displayed after it. In the `rg` pane, the context column contains the query matches and their context. To search them, type `%`, which switches the active filtering column.

# Tools

### Shell integration

Only zsh is supported for now.

The output of `fs :tool shell`, when sourced, provides the jump and jump+open functions:

The jump function (`z`) is a replacement for `cd`, except that incomplete queries are matched to a most likely destination drawn from the unified f:ist database.

> [!NOTE]
>
> In addition, a couple special queries can be used to start an interactive search. Ultimately, the full behavior is as follows:
>
> the only argument is a valid path: `cd`.
> no arguments: interactively select from history.
> last argument is `.` : interactively search subdirectories of the best match.
> last argument is `./`: interactively navigate the best match [^2].
> otherwise: cd into the best match[^1] for the search term (if one exists).

[^1]: See: [zoxide](https://github.com/ajeetdsouza/zoxide)

[^2]: If you have [aliases](#aliases) enabled, this is also just `Z`.

The jump+open function (`zz`) is an analogous replacement for [`lessfilter edit`](#lessfilter): if the query head exists, it opens the target(s) in the editor. Otherwise the query is passed to `z`, and the editor opens in the destination.

##### Additional

The `--aliases` flag can be enabled to additionally output a few simple alias definitions:

- [lessfilter](#lessfilter)
- lz: directory display
- l: lessfilter (display preset)
- la: lessfilter (extended preset)
- ll: lessfilter (info preset)
- n: edit (lessfilter with edit preset)
- o: [open](#app)
- Z: `z`, then navigate
- `zf`: recent files history

For speed and safety, it is recommended pass your actual shell through to `--shell`.[^4] Another optimization you can make is to cache the generated command: my [zcomet fork](#https://github.com/Squirreljetpack/zcomet) supports this.

### Lessfilter

The previewer is controlled by the lessfilter tool.

The lessfilter tool dispatches to 10 presets:

- preview: For the preview pane
- display: For terminal display
- extended: For terminal interaction/verbose display
- info: Metadata/raw info
- extract: extract document contents with [kreuzberg](#https://github.com/kreuzberg-dev)
- open: System open
- alternate: Alternate (custom) open
- edit: For editing

Each preset is configured by a rules table; each rule is a pair (Actions, Patterns); and for a given file, the rule whose patterns score the highest is selected -- its actions are invoked on the target file.

The patterns can be prefixed with a score modifier which dictates how the score is modified by a successful match of the pattern - if this is omitted, the default score modifier for the pattern is used.

The score modifiers are:

- Add/Sub (n): Add/Sub (n) to the current score.
- Max/Min (n): Take the max/min of the current score with (n) for the new score.
- Req: Set the score to 0 if the test fails.

The patterns are:

- Glob: (default score: `Max(100)`)
- Child: (default score: `Max(50)`)
- Mime: (default score: `Max(20)`)
- Cat: (default score: `Max(20)`)
- Ext: (default score: `Max(10)`)
- Have: (default score: `Req`)
- Filetype: (default score: `Req`)

Though the syntax has many parts, configuration should be fairly straightforward. F:ist comes with a sane set of defaults with wide coverage for a variety of filetypes, and declaring overrides is as simple as declaring the desired action together with the conditions which it requires. For example:

```toml
preview = [
  # ...
  # On an file with mime-type sqlite-3 and a system with sqlite3, this rule gets a score of 20.
  [ [ "sqlite" ], [ "application/vnd.sqlite3", "have:sqlite3" ] ],
  # ...
]

# When invoking the edit action (in `fist` or through the `n` alias),
# any file belonging to this category will be opened with the system's default preferred application.
# Note that since this rule has minimal priority (at most 1), any subsequent rule will override it.
edit = [
  [ [ "Open" ], [ "1|cat:document", "1|cat:spreadsheet", "1|cat:email", "1|cat:academic" ] ],
]
```

The built-in actions are:

- Text
- Image
- Metadata
- Directory
- Header
- None
- Open

Additional actions can be defined with shell syntax. For example:

```toml
[rules]
alternate = [
  [["code"], ["*/*"]],
]
[actions]
code = 'code --add {}'
```

###### Addditional notes

- Image display requires [chafa](https://github.com/hpjansson/chafa).
- document preview (i.e. pdf) requires [kreuzberg](#https://github.com/kreuzberg-dev)

# Additional

### Dependencies

- fd-find
- eza
- ripgrep
- bat (preview)
- chafa (preview)

Conversely, fist integrates into [CommandSpace](https://github.com/Squirreljetpack/command-space), which you may also enjoy checking out.

### Notes

- The `New` action creates a directory if the target ends with a path seperator[^1].

- The process which runs the command that spawns programs can be relegated to a process manager. For example, using [pueue](https://github.com/Nukesor/pueue):

```toml
# config.toml

[misc]
spawn_with = ["pueue", "add", "-g", "apps", "--"]
```

[^1]: `/` on unix and `\` on windows
