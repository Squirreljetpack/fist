# F:ist

F:ist is a fast and intuitive search tool for the filesystem.

// video

# Installation

`cargo install fd-find eza fist`

Call as:
- `fs`: Directory navigation
- `fs [..paths] pattern`: interactive find
- `generate_paths | fs`: enriched fuzzy searching of paths

fs also integrates with your [shell](#shell-integration), as well as with [fzs](https://github.com/Squirreljetpack/fzs).



# Commands

### Default bindings overview
- `Up`/`Down`: Navigate (or `Up` in the initial position to to enter prompt).
- `Left`/`Right`: Back/Enter.
- `Enter`: Default (system) open.
- `alt-enter`: Open in terminal.
- `Tab`: Toggle select.

---

- `ctrl-f`/`alt-f`: Find/Find text.
- `ctrl-g`: History view (Folders and files).
- `ctrl-z`/`ctrl-y`: Undo/Redo

---

- `ctrl-x`/`ctrl-c`/`ctrl-v`: Cut, Copy, Paste.
- `ctrl-s`: Open stack.
- `alt-s`: Open filters.
- `ctrl-e`: Open menu.

---

- `ctrl-o`: Default open without exiting.
- `ctrl-l`: Full preview.
- `/` and `~`: Jump to home
- `?`: toggle preview

For more information on bindings, see [matchmaker](https://github.com/Squirreljetpack/matchmaker).

# Shell integration

#### Jump

# Additional

# Dependencies
- fd-find
- eza
- ripgrep 
- matchmaker
