---
name: fist-usage
description: Operates the F:ist interactive TUI - navigating panes, selecting, copying/moving/renaming files, working the queue overlay, stashes and history, previews and pagers, metadata display, and the options overlay. Use when explaining or scripting everyday fs workflows, writing cheatsheets or docs, or advising how to accomplish a file task inside the f:st interface.
compatibility: Applies to the interactive `fs` TUI. Keys reflect shipped defaults (`mm.toml` + built-ins); every bind is rebindable, so verify against the user's config when in doubt (press `alt-h` in-app for live help).
---

# Using the f:ist TUI

Use this skill to answer "how do I do X in fs" questions with concrete keys and flows. The app is a stack of panes; `Undo`/`Redo` move between them, and most operations act on the **selection** (multi), the **cursor item**, or — while the cursor is disabled in the prompt state — the **current directory**.

## Mental model

- Results list + query prompt. Up past the first row (or Down past the last) enters the *prompt*: the input becomes your current directory (shown as the cwd path) and actions apply to that directory.
- With prompt locking on (default binds), `left`/`right` are `Parent`/`Advance`; inside the prompt they revert to text editing and `shift-left`/`shift-right` carry the pane actions instead. `alt-space` toggles the lock. If this feels unpredictable, remove the `prompt^^...` binds from `mm.toml`.
- Every pane has an Options overlay (`alt-p`) for filtering/sorting; the menu overlay (`alt-e`) lists contextual file operations plus custom actions.

## Getting around

| Key(s)                       | Action                                             |
| ---------------------------- | -------------------------------------------------- |
| `up` / `down`                | Move; past either end enters/leaves the prompt     |
| `left` / `right`             | Parent directory / enter item                      |
| `shift-left` / `shift-right` | Text editing in the prompt; pane actions otherwise |
| `ctrl-f`, `ctrl-r`           | Find (fd) / Search (ripgrep) panes rooted here     |
| `ctrl-g`                     | History (recent files/folders)                     |
| `ctrl-z`, `alt-z`            | Undo / Redo (navigate the pane stack)              |
| `` ctrl-` ``                 | Jump home/root toggle                              |
| `ctrl-0`                     | Enter/leave prompt                                 |
| `ctrl-1..9`                  | Jump to result N (and accept, per config)          |
| `alt-space`                  | Toggle prompt lock                                 |
| `alt-h`                      | Bindings help                                      |

## Selecting

- `tab`: toggle selection under cursor (and move down).
- `ctrl-a`: cycle selections; `ctrl-shift-a`: clear all.
- `enter`: open/default action; with a selection, multi-accept opens all.
- `alt-enter`: print paths to stdout and exit (the shell-integration contract).

## Everyday file operations

| Task              | Key(s)                    | Notes                                                     |
| ----------------- | ------------------------- | --------------------------------------------------------- |
| Copy              | `ctrl-c`                  | Enqueues `copy` rows + system clipboard                   |
| Cut               | `ctrl-x`                  | Enqueues `move` rows                                      |
| Paste             | `ctrl-v`                  | Executes queued copy/move/symlink into the Nav directory  |
| Copy path         | `ctrl-y`                  | Full paths of selection/cursor/cwd                        |
| New file / folder | `ctrl-n` / `ctrl-shift-n` | Menu input bar; trailing `/` creates a directory          |
| Rename            | `f2` or `ctrl-shift-r`    | Input pre-filled, cursor on the file stem                 |
| Trash / Delete    | `delete` / `shift-delete` | Confirmed; in history/stash panes removes only the record |
| Set alias         | via menu (`alt-e`)        | Display alias for a file                                  |
| Drop to shell     | `ctrl-esc`                | `Execute($SHELL)` in the current directory                |

## The menu overlay (`alt-e`)

Lists builtin items (`new`, `rename`, `move`, `copy`, `symlink`, `goto`, `trash`, `delete`, `open`, `open with`) followed by installed custom actions. Type an entry's highlighted letter (e.g. `w` for open-with) or filter and press Enter. Conditions decide visibility: entries appear only when the current targets qualify.

## Working the queue (`alt-u`)

Rows show kind, source, destination (`To`), and progress.

- `up`/`down` navigate; `tab` multi-selects rows.
- `shift-up`/`shift-down` reorder rows.
- `delete` cancels/removes the row.
- `enter` executes the selected rows (or the current one).
- `alt-e` edits a single-path row's Source; rename key (`ctrl-shift-r`) edits its destination.
- `ctrl-z`/`alt-z` cycles the `[kind: ...]` filter to narrow to one queue kind.
- `esc` closes.

Binds worth knowing: `Enqueue(kind)`, `ExecuteQueue([selector])`, `ClearQueue([selector])`; selectors are `all`, `builtins` (= Paste), `first`, `last`, or an action key.

## Stashes and bookmarks

`Stash(name)` is smart: push the selection when there is one, back out when already inside, otherwise open the pane. Defaults: `alt-s` = unnamed stash, `alt-b` = persistent `bookmark`. `PushStash(name)`/`OpenStash(name)` do the two halves explicitly. Stashes are scratch space for cross-directory workflows: collect files from anywhere, then run a menu action (compare, archive) over the stash contents. Per-stash settings (`panes.stashes`) choose transient vs database-backed behavior and duplicate handling.

## History and apps

- Files/folders/applications you touch are recorded in a local SQLite db; `ctrl-g` browses them ranked by frecency. `Delete` there removes the history entry, not the file.
- The Apps pane (`fs :o`, the `open with` menu item, or a bound `App` action) collects pending files and picks the launching program.
- Shell integration provides `z` (smart cd), widget functions, and aliases: `l` display, `la` extended, `ll` info, `n` edit, `lz` listing, `o` open, `zf` recent files.

## Preview, metadata, pager

- `?` toggles the preview pane; `shift-up/down` scrolls it; preview layouts cycle with `ctrl-/` and `ctrl-shift-/`.
- `alt-/` informative preview (metadata preset); `alt-shift-/` quick terminal display; `ctrl-l` maximizes the preview; `alt-l` maximizes the interactive/extended view.
- Metadata appears contextually: time/size columns engage once an explicit sort does, and the info surfaces mediainfo-style details for media.
- Anything executed via `ExecPaged`/`LFPaged` lands in the built-in pager: `q` quit, `j/k` lines, space page, `g/G` top/bottom, `h/l` horizontal, `/` search, `n/p` next/prev match, `ctrl-l` line numbers.
- Entering supported archives transparently extracts a temporary skeleton; `Parent` exits back to the archive's location.

## The options overlay (`alt-p`)

Three columns — filters, sort, pane-specific — navigable with arrows, toggle with Enter or hotkeys, close with `q`:

- Filters: `h` hidden, `H` hidden-only, `I` ignored, `d`/`D` dirs↔files, `a` all.
- Sort: `n` name, `s` size, `t` time cycle (mtime→atime→none); db/history panes use `c` (count) and `f` (frecency) instead. The active sort shows on the right; sorting is soft, never losing query relevance.
- Search-pane extras: `b/B`, `d/D`, `c/C` shrink/grow match context (before/after/both), `e` case mode, `1` one-line matches, `r` regex vs fixed strings.

Outside the overlay, `ctrl-d` cycles dirs/files/all visibility and `ctrl-s` toggles hidden.

## Customization pointers

- Binds live in `mm.toml`'s `[binds]`; right-hand sides are plain strings parsed into actions — e.g. `"ExecuteQueue(copy)"`, `"LFPaged(Info)"`, `"Stash(bookmark)"`, `"Execute($SHELL)"`, or any lessfilter preset name (`"Info"`).
- Pane defaults (visibility/sort/prompt/preview) live under `panes.*` in `config.toml`; interface behaviors under `interface.*` (`advance_command` decides what Enter does on files). `panes.nav.ignore_patterns` lists globs the Nav pane hides unless `all` visibility is engaged.
- Menu actions are added via `actions.toml`/`actions/` — see the fist-menu-actions skill.

## Common gotchas

- **Arrow keys typed characters instead of navigating:** you were inside the locked prompt; use `shift-left/right` or leave the prompt (`up`/`down`/`alt-space`).
- **Terminal froze on `ctrl-s`:** flow control; `stty -ixon` in your shell rc.
- **Paste did nothing:** paste targets the Nav directory; make sure a nav pane is beneath you in the pane stack, and rows have destinations.
- **Trash removed "too much" (db-backed panes):** on stashes and the files/folders/apps history panes, Trash removes only the records (*Remove ... from history/stash*); `Delete` always does. Set `interface.allow_trash_db_items = true` if Trash should operate on the real items there.
- **Stream/custom panes refuse reload:** stdin-fed listings cannot be re-fetched; re-run the command.
- **A key did something unexpected:** check `alt-h` first — user overrides in `mm.toml` win over this document.
