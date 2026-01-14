# F:ist

F:ist is a fast and intuitive search tool for the filesystem.

// video

# Installation

```shell
# dependencies
cargo install fd-find eza ripgrep

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

### Default bindings overview

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

# Shell integration

#### Jump

# Additional

### Dependencies

- fd-find
- eza
- ripgrep

Conversely, fist integrates into [CommandSpace](https://github.com/Squirreljetpack/command-space), which you may also like to check out.

### Notes

- The `New` action creates a directory if the target ends with a path seperator[^1].

- The process which runs the command that spawns programs can be relegated to a process manager. For example, using [pueue](https://github.com/Nukesor/pueue):

```toml
# config.toml

[misc]
spawn_with = ["pueue", "add", "-g", "apps", "--"]
```


[^1]: `/` on unix and `\` on windows