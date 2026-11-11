# Actions

Key bindings live in the `[binds]` table of `mm.toml`. Each entry maps a
trigger to one action string, or to a list of actions that run in order:

```toml
[binds]
"ctrl-n" = "New"
"tab" = ["ToggleSelection", "Down"]
```

Action names are case-insensitive. Arguments are written in parentheses and
may be omitted when an action has a default: `Down` and `Down(1)` are the
same. Strings inside arguments are passed through literally, so
`Execute(echo hello)` runs `echo hello` in your shell.

Menu actions (`actions.toml`) are a separate, Lua-based mechanism; see
[Menu Actions](menu-actions.md).

## Panes & navigation

| Action               | Description                                                                                                                                                                                                                                                |
| -------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Advance`            | Enter the current file or directory. Files open through the lessfilter edit preset by default.                                                                                                                                                             |
| `Parent`             | Go to the parent of the current directory, or of the current item when the pane has no directory.                                                                                                                                                          |
| `Find`               | Open the find pane in the current directory.                                                                                                                                                                                                        |
| `Search`             | Open the full-text search pane. Inside the search pane, toggle between query mode and filter mode.                                                                                                                                                         |
| `History`            | Open recent files. Inside a history pane, switch between the file and folder lists.                                                                                                                                                                        |
| `App`                | Open the application launcher pane. No-op when already in it.                                                                                                                                                                                              |
| `Undo`               | Go back in the pane stack.                                                                                                                                                                                                                                 |
| `Redo`               | Go forward in the pane stack.                                                                                                                                                                                                                              |
| `Jump(paths)`        | Jump to one of the given directories, cycling to the next path when the current directory is already one of them. Paths are comma-separated and resolve relative to your home directory: `Jump(notes, src)`. With no argument, cycles between `~` and `/`. |
| `JumpOrAccept(n)`    | Jump to the *n*-th item and accept it when the cursor is already there. `JumpOrAccept(0)` targets the current directory. The `ctrl-1`…`ctrl-9` defaults use this.                                                                                          |
| `LockPrompt([bool])` | Enter or leave the query bar. With no argument, toggles it.                                                                                                                                                                                                |
| `ShowMounts`         | Show the mounts picker: the volumes a mounted image or extracted archive was split into.                                                                                                                                                                   |
| `SetSort(label)`     | Set the sort order by its display label: `name`, `mtime`, `atime`, `size`, or `none`. Database panes label `none` as `frecency`, `size` as `count`, and `mtime` as `none`.                                                                                 |

## Display & overlays

| Action         | Description                                                                                                                           |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| `ShowMenu`     | Open the menu of contextual and custom actions.                                                                                       |
| `ShowOptions`  | Open the options overlay (visibility, sorting, pane settings).                                                                        |
| `ShowQueue`    | Open the queue overlay. In the app pane, opens the pending-files view instead.                                                        |
| `Help`         | Show the keybinding help for the current context.                                                                                     |
| `CycleFilter`  | Next filter: file/directory visibility in filesystem panes, sort order in history and stash panes, case mode in the search pane.      |
| `ToggleFilter` | Toggle hidden-file visibility in filesystem panes, frecency/recency in history panes, regex/fixed-string matching in the search pane. |

## Files & directories

| Action                 | Description                                                                                                                                                                           |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Move`                 | Enqueue the selection (or the current directory in prompt mode) under the `move` kind, and cut the paths to the system clipboard when `interface.move_files_to_clipboard` is enabled. |
| `Copy`                 | Enqueue the selection under the `copy` kind, and copy the files to the system clipboard when `interface.copy_files_to_clipboard` is enabled.                                          |
| `CopyPath`             | Copy the selected paths as text.                                                                                                                                                      |
| `CopyFiles`            | Copy the selected files themselves, for pasting into a file manager.                                                                                                                  |
| `New`                  | Create a file relative to the current item's parent. A trailing separator creates a directory.                                                                                        |
| `NewFolder`            | Create a directory relative to the current item's parent.                                                                                                                             |
| `Rename`               | Rename the item under the cursor, or the current directory in prompt mode.                                                                                                            |
| `Trash([no_confirm])`  | Move the item to the system trash. Pass `true` to skip the confirmation prompt: `Trash(true)`.                                                                                        |
| `Delete([no_confirm])` | Permanently delete the item. Pass `true` to skip the confirmation prompt.                                                                                                             |
| `Paste([destination])` | Execute all pending `copy`, `move`, and `symlink` rows into the current directory, or into an absolute destination path.                                                              |

In the database-backed panes (history and stashes), `Trash` and `Delete` can
remove the database record instead of touching the filesystem; see
`interface.remove_db_record_action` in [config.toml](config.md).

## Queue & stashes

| Action                     | Description                                                                                                                       |
| -------------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| `Enqueue(kind)`            | Enqueue the current selection under `kind`: a builtin queue kind (`copy`, `move`, `symlink`, `none`) or the key of a menu action. |
| `ExecuteQueue([selector])` | Execute the queued rows selected by `selector` into the current directory.                                                        |
| `ClearQueue([selector])`   | Remove the queued rows selected by `selector`, after confirmation.                                                                |
| `Paste([destination])`     | Execute the builtin transfer rows. Listed here because it is the ordinary way to drain the queue.                                 |
| `OpenStash([name])`        | Switch to the named stash pane. The unnamed stash is `OpenStash` or `OpenStash("")`.                                              |
| `PushStash([name])`        | Add the selection, or the current directory in prompt mode, to the named stash without switching panes.                           |
| `Stash([name])`            | Smart stash: push the selection when there is one; otherwise open the stash, or leave it when already there.                      |

`selector` defaults to `all` and may also be `builtins` (the `copy`, `move`,
and `symlink` kinds), `first`, `last`, or a specific queue kind such as
`ExecuteQueue(copy)` or `ExecuteQueue(my-action)`.

## Shell commands

Commands are formatted with the [output templates](../docs/TEMPLATING.md)
before they run, so they can refer to the current item and environment.

| Action                  | Description                                                                                                         |
| ----------------------- | ------------------------------------------------------------------------------------------------------------------- |
| `Execute(cmd)`          | Run the command in the current directory and wait for it to exit. `Exec(cmd)` is an alias.                          |
| `ExecuteOrConfirm(cmd)` | Run and wait; when it exits non-zero, wait for Enter before returning to the TUI. `ExecOrConfirm(cmd)` is an alias. |
| `ExecPaged(cmd)`        | Run with stdout streamed into the pager. `ExecutePaged(cmd)` is an alias.                                           |
| `ExecTTY(cmd)`          | Run with stdin, stdout, and stderr connected to the terminal. `ExecuteTTY(cmd)` is an alias.                        |
| `ExecDetached(cmd)`     | Run detached with no output and return immediately. `ExecuteDetached(cmd)` is an alias.                             |
| `ExecSilent(cmd)`       | Run with no output and return immediately, without detaching. `ExecuteSilent(cmd)` is an alias.                     |
| `CopyCommand(cmd)`      | Run the command and copy its stdout to the clipboard.                                                               |
| `CopyCommandAsync(cmd)` | Run the command in the background and copy its stdout when it completes.                                            |

`ExecuteWait(cmd)` also exists but is not accepted in `[binds]`; the
confirmation behavior is reached through `ExecuteOrConfirm`.

## Lessfilter

A bare preset name is the shorthand for running that preset as a lessfilter
action. The presets are `preview`, `display`, `extended`, `info`, `open`,
`alternate`, `alternate2`, and `edit`.

| Action                         | Description                                                                                                                                   |
| ------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------- |
| `Preview` (or any preset name) | Run the preset on the current item and show its output normally.                                                                              |
| `Lessfilter(preset)`           | Explicit form of the above.                                                                                                                   |
| `LFPaged(preset)`              | Run the preset and page its output. `LessfilterPaged(preset)` is an alias.                                                                    |
| `LFPreview(preset)`            | Run the preset into the preview pane, toggling back to the default preview when it is already shown. `LessfilterPreview(preset)` is an alias. |

## Selection & exit

| Action                       | Description                                                                                                                   |
| ---------------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| `Select`                     | Add the current item to the selection.                                                                                        |
| `Deselect`                   | Remove the current item from the selection.                                                                                   |
| `ToggleSelection` / `Toggle` | Toggle the current item's selection state.                                                                                    |
| `CycleSelections` / `Cycle`  | Toggle every visible item's selection state.                                                                                  |
| `ClearSelections` / `Clear`  | Clear all selections.                                                                                                         |
| `Accept`                     | Accept the current item or selection and exit.                                                                                |
| `Quit([code])`               | Exit with an optional status code, default `0`.                                                                               |
| `Print([text])`              | Print the current item or selection through the output template and exit. With a non-empty argument, print that text instead. |
| `PrintKey`                   | Print the key that triggered the binding.                                                                                     |

## Result navigation

| Action                        | Description                                              |
| ----------------------------- | -------------------------------------------------------- |
| `Up([n])`                     | Move the cursor up by `n` rows, default `1`.             |
| `Down([n])`                   | Move the cursor down by `n` rows, default `1`.           |
| `Pos(idx)`                    | Move to an absolute row. `-1` selects the last row.      |
| `HalfPageUp` / `HalfPageDown` | Move by half a screen.                                   |
| `HScroll([n])`                | Scroll horizontally by `n`; `0` resets.                  |
| `VScroll([n])`                | Scroll the current result vertically by `n`; `0` resets. |
| `ToggleWrap`                  | Toggle wrapping in the results list.                     |
| `ToggleHeaderWrap`            | Toggle wrapping in the header.                           |

## Preview

| Action                                      | Description                                                                                |
| ------------------------------------------- | ------------------------------------------------------------------------------------------ |
| `NextPreview` / `PrevPreview`               | Cycle through the configured preview layouts.                                              |
| `Preview(cmd)`                              | Show the given command's output in the preview.                                            |
| `SetPreview([idx])`                         | Set the preview layout by index; without an index, restore the current layout's command.   |
| `SwitchPreview([idx])`                      | Switch to a layout; without an index, toggle preview visibility.                           |
| `TogglePreviewWrap`                         | Toggle wrapping in the preview.                                                            |
| `PreviewUp([n])` / `PreviewDown([n])`       | Scroll the preview by `n` rows, default `1`.                                               |
| `PreviewHalfPageUp` / `PreviewHalfPageDown` | Scroll the preview by half a screen.                                                       |
| `ExpandPreview([n])` / `ShrinkPreview([n])` | Grow or shrink the preview by `n`, default `1`.                                            |
| `PreviewHScroll([n])`                       | Persistently scroll the preview horizontally; `0` resets. Pans a shown image.              |
| `PreviewScroll([n])`                        | Persistently scroll the preview vertically; `0` resets. Zooms a shown image by `n` tenths. |
| `PreviewJump`                               | Jump between the preview's start, end, and initial position.                               |

## Input editing

| Action                               | Description                                                   |
| ------------------------------------ | ------------------------------------------------------------- |
| `ForwardChar` / `BackwardChar`       | Move the query cursor by one character.                       |
| `ForwardWord` / `BackwardWord`       | Move the query cursor by one word.                            |
| `DeleteChar` / `DeleteWord`          | Delete the character or word at the cursor.                   |
| `DeleteLineStart` / `DeleteLineEnd`  | Delete from the cursor to the start or end of the query.      |
| `ClearQuery` / `Cancel`              | Clear the query.                                              |
| `ClearQueryLeft` / `ClearQueryRight` | Clear the query before or after the cursor.                   |
| `SetQuery(text)`                     | Replace the query.                                            |
| `InsertQuery(text)`                  | Insert text at the query cursor.                              |
| `QueryPos(n)`                        | Move the query cursor to position `n`; `-1` moves to the end. |

## Columns

| Action                                        | Description                                                                              |
| --------------------------------------------- | ---------------------------------------------------------------------------------------- |
| `NextColumn` / `PrevColumn`                   | Focus the next or previous column.                                                       |
| `SwitchColumn(name)`                          | Focus a column by name or index.                                                         |
| `HideColumn([name])` / `UnhideColumn([name])` | Hide or unhide a column; without an argument, the active or most recently hidden column. |
| `ExpandColumn([n])` / `ShrinkColumn([n])`     | Widen or narrow a column by one, or the *n*-th non-hidden column when given.             |

## Scripting & state

| Action              | Description                                                                   |
| ------------------- | ----------------------------------------------------------------------------- |
| `ExecuteAsync(cmd)` | Run in the background; the next action in the batch starts when it completes. |
| `ExecuteThen(cmd)`  | Like `ExecuteAsync`, but subsequent actions run only when it succeeds.        |
| `Become(cmd)`       | Replace the f:st process with the command.                                    |
| `BecomeSilent(cmd)` | Replace the process without clearing the screen first.                        |
| `Reload([cmd])`     | Reload the current pane. With a command, rerun the pane using it.             |
| `Store(value)`      | Set `MM_STORE`, the value available to command templates.                     |
| `Redraw`            | Force a full redraw.                                                          |

## Advanced

f:st uses the same system for bindings as matchmaker, with only the app-specific actions differing. This entails some niche but maybe cool features:

#### Triggers

A trigger is the key, mouse event, event, or named alias that activates a
binding.

- **Keys** use `ctrl-`, `alt-`, `shift-`, and `super-` prefixes:
  `ctrl-c`, `alt-enter`, `shift-up`, `alt-shift-right`, `f2`, `enter`, `esc`,
  `space`, `tab`, `backspace`, `pageup`, `pagedown`, `home`, `end`. A literal
  key is written as itself: `?`, `/`, `` ` ``.
- **Mouse** triggers are `left`, `middle`, `right`, `scrollup`, `scrolldown`,
  `scrollleft`, and `scrollright`, optionally with `ctrl+`, `alt+`, or
  `shift+` (for example `ctrl+scrollup`).
- **Semantic aliases** start with `@`. Define one by binding the alias, then
  use it from keys or other actions:

  ```toml
  [binds]
  "@open-config" = "ExecTTY($EDITOR ~/.config/fist/config.toml)"
  "ctrl-o" = "@open-config"
  ```

- **Events** are named after the point in the UI they describe: `Start`,
  `Synced`, `Resynced`, `Reloaded`, `Restarted`, `QueryChange`,
  `CursorChange`, `CursorLost`, `PreviewChange`, `PreviewSet`,
  `OverlayChange`, `PreviewStarted`, `PreviewFinished`, `Resize`, `Redraw`,
  `Tick`, `Pause`, and `Resume`.

`fs :tool showbinds` prints the resolved bindings; `alt-h` shows them in the
app.

#### Modes

A trigger can be restricted to a pane or UI state by prefixing it with a mode
pattern and `^^`:

```toml
[binds]
"nav^^alt-n" = "Execute($EDITOR .)"
"prompt^^ctrl-h" = "DeleteWord"
```

f:st sets the active mode from the pane:

| Mode     | Active in                      |
| -------- | ------------------------------ |
| `custom` | Custom stream panes.           |
| `find`   | The find pane.                 |
| `app`    | The application launcher pane. |
| `search` | The full-text search pane.     |
| `files`  | The recent-files pane.         |
| `dirs`   | The recent-folders pane.       |
| `nav`    | The navigation pane.           |
| `stash`  | Stash panes.                   |
| `mount`  | The mounts pane.               |

The `prompt` tag is added while the query bar is active. Patterns may be
combined with commas (all must match) and negated with `!`: `nav,!prompt^^j`
only applies in the nav pane when the query bar is closed.

A binding without a mode is a fallback. When several bindings match the same
trigger, the one with more mode patterns wins; ties go to the definition that
appears later in the file, so your `mm.toml` overrides the built-in defaults.
