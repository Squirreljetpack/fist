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

For a full list of binds within the app, type `ctrl-shift-h`.\
For more information on bindings, see [matchmaker](https://github.com/Squirreljetpack/matchmaker).

# Panes

F:ist records the files, directories and applications that you've visited in a local database, using it to sort the results in the `App` and `History` panes by relevance.

# Tools

### Shell integration

Only zsh is supported for now.

The output of `fs :tool shell`, when sourced, provides the jump and jump+open functions:

The jump function (`z`) is a replacement for `cd`, except that incomplete queries are matched to a most likely destination drawn from the unified f:ist database.

> [!INFO]
> In addition, a couple special queries can be used to start an interactive search. Ultimately, the full behavior is as follows:
>
> the only argument is a valid path: `cd`.
> no arguments: interactively select from history.
> last argument is `.` : interactively search subdirectories of the best match.
> last argument ends with `/`: interactively navigate the best match.
> otherwise: cd into the best match[^1] for the search term (if one exists).

[^1]: See: [zoxide](https://github.com/ajeetdsouza/zoxide)

The jump+open function (`zz`) is an analogous replacement for [`lessfilter edit`](#lessfilter): if the query head exists, it opens the target(s) in the editor. Otherwise the query is passed to `z`, and the editor opens in the destination.

### Lessfilter

The previewer is controlled by the lessfilter tool.

The lessfilter tool dispatches to 9 presets:

- preview: For the preview pane
- display: For terminal display
- extended: For terminal interaction/verbose display
- info: Metadata/raw info
- open: System open
- alternate: Alternate (custom) open
- edit: For editing

Each preset is configured by a rules table; each rule is a pair (Actions, Patterns); and for a given file, the rule whose patterns score the highest is selected -- its actions are invoked on the target file.

The patterns can be prefixed with a score modifier which dictates how the score is modified by a successful match of the pattern - if this is omitted, the default score modifier for the pattern is used.

The score modifiers are:

- Add:
- Sub:
- Min:
- Max:

The patterns are:

- Glob
- Ext
- Child
- Mime
- Have

For example:

```toml

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

Image display requires [chafa](https://github.com/hpjansson/chafa).

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
