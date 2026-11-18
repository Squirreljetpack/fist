# Menu Actions

The context menu (`alt-e` or `ctrl-e`) extends f:st with custom Lua scripts alongside built-in file operations—converters, diffing helpers, archive builders, bulk renamers, or sharing tools. Conditions keep the list scoped to what fits the cursor row or marked selection, while fuzzy filtering and single-character aliases make it quick to navigate.

f:st loads actions from `~/.config/fist/actions.toml` (seeded by `fs --dump-config`) and every `*.toml` file under `~/.config/fist/actions/`, including nested subdirectories. `actions.toml` loads first, followed by files in `actions/` ordered from deepest subdirectory to shallowest and lexicographically within each depth—so numeric prefixes like `01-` control menu order, and insertion order within each file sets display order. Every action key must be unique across all files and cannot reuse reserved queue or selector names (`copy`, `move`, `symlink`, `none`, `all`, `builtins`, `first`, `last`, `default`, or an empty string).

Press `alt-h` inside the context menu to open its contextual help in the pager; any visible custom action with a `help` string appears under Action Guidance. Run `fs :tool check` after editing your action files to validate TOML syntax, compile every Lua script, and inspect the loaded actions, and use `fs :tool action <name>` to test an action directly from your shell without opening the TUI.

## Fields

Each entry in an actions file is a TOML table keyed by the action's name (such as `["diff"]` or `["to webp"]`):

| Field           | Type                           | Default          | Meaning                                               |
| --------------- | ------------------------------ | ---------------- | ----------------------------------------------------- |
| `command`       | string                         | *(required)*     | Inline Lua script or `@path` to a `.lua` file         |
| `condition`     | condition or `{ any = [...] }` | always visible   | Visibility and target rule evaluated on menu open     |
| `strategy`      | string or `{ QueueBatch = n }` | `"ExecSilent"`   | How the script executes                               |
| `prefix`        | string                         | `":"`            | Group label shown as `<prefix>: <key>`                |
| `alias`         | string                         | none             | Hotkey shown in the alias column for instant launch   |
| `help`          | string                         | `""`             | Description shown in menu help (`alt-h`) when visible |
| `requires_dest` | bool                           | `false`          | Require a non-empty destination for queued execution  |
| `close`         | bool                           | strategy default | Override whether choosing the action closes the menu  |

When `command` starts with `@`, f:st reads the script from disk rather than compiling the string directly; relative paths after `@` resolve from `~/.config/fist/actions/`.

### Prefixes

Organizing actions across topic files under `~/.config/fist/actions/`—such as `image.toml` or `git.toml`—lets `prefix` group related entries automatically in the menu without repeating the category in every key. The default `":"` expands to the declaring file's stem (`image: to webp` for `["to webp"]` in `image.toml`), an explicit string overrides that label, and `prefix = ""` omits the prefix so the row displays the bare key.

## Strategies

Different tasks need different relationships with the terminal: interactive prompts and editors must own `/dev/tty`, diffs belong in a pager, fast side effects should finish quietly in the background, and heavy batch jobs benefit from sitting in the queue where you can adjust destinations or watch progress.

| Strategy             | Execution                                                       | Interactive `input` |
| -------------------- | --------------------------------------------------------------- | ------------------- |
| `"Exec"`             | Foreground on `/dev/tty`; resumes TUI on exit                   | yes                 |
| `"ExecWait"`         | Foreground on `/dev/tty`; waits for `enter` before redrawing    | yes                 |
| `"ExecSilent"`       | Background thread; reports via toasts                           | no                  |
| `"ExecPaged"`        | Foreground; pipes stdout into the interactive [pager](pager.md) | no                  |
| `"Queue"`            | Enqueues one multi-path row into the queue                      | no                  |
| `{ QueueBatch = n }` | Enqueues rows of at most `n` paths each                         | no                  |
| `"Enqueue"`          | Enqueues one multi-path row and starts it immediately           | no                  |

`Execute`, `ExecuteWait`, `ExecuteSilent`, and `ExecutePaged` are accepted as aliases for the four `Exec*` spellings. Every strategy except `Queue` and `{ QueueBatch = n }` closes the context menu after launching; set `close = true` or `close = false` to override that default.

`Queue`, `{ QueueBatch = n }`, and `Enqueue` create queue rows whose kind is the action's key. Keeping the menu open for `Queue` and `QueueBatch` lets you queue several operations before switching to the queue overlay (`alt-u`) to fill destinations and dispatch them. `Enqueue` starts immediately with an empty destination, so it cannot be combined with `requires_dest = true`.

## Lua environment

Each menu action runs inside a fresh, isolated Lua 5.4 VM so concurrent background and queued runs never share or leak global state. Scripts execute in `fs`'s process working directory rather than the active pane's directory, so commands that rely on a specific cwd should pass `{ cwd = ... }` to `proc.run` / `proc.spawn` / `proc.exec` or `cd` explicitly.

Alongside the six standard modules (`path`, `fs`, `proc`, `util`, `input`, and `lunacolors`) documented in [Lua](lua.md), f:st injects these globals and vararg (`...`) parameters:

| Symbol                            | Value                                                                                     | Availability               |
| --------------------------------- | ----------------------------------------------------------------------------------------- | -------------------------- |
| `paths`                           | Array of resolved absolute path strings (always a table)                                  | Always                     |
| `to`                              | Destination string (`""` unless queued with a destination or passed `--to`)               | Always                     |
| `nav_cwd`                         | Active Nav pane directory string, or `nil` outside a Nav pane                             | Always                     |
| `toast(style, msg)`               | Display a footer notification (`"normal"`, `"info"`, `"success"`, `"warning"`, `"error"`) | Always                     |
| `toast_push(style, prefix, item)` | Append an item to a grouped footer notification                                           | Always                     |
| `set_progress(n)`                 | Set the queue row progress bar (`0`–`255`)                                                | Queued strategies only     |
| `input`, `proc.editor`            | Terminal prompts and `$EDITOR` launcher                                                   | `Exec` and `ExecWait` only |

`os.exit([code])` is overridden per VM so calling it aborts only the script (raising a runtime error carrying `code`) without terminating `fs`.

## Conditions

Conditions keep the menu focused on what actually applies to the files you are looking at, and they also control which paths end up in `paths` when an action runs. Without a `condition`, an action is always visible and targets the marked selections, falling back to the cursor row (or the pane directory when the prompt is focused).

Write `{ any = [ cond1, cond2 ] }` to make an action visible whenever at least one branch matches. Any branch that requires a missing program via `have:<prog>` is pruned when actions load; if no viable branch remains, the action stays hidden for that session while keeping its key reserved.

### Positional sequence

A positional sequence is a TOML array of rules—for example `["type:f", "type:f"]` or `[["type:f", "ext:json"], "type:d"]`. It matches only when the number of marked selections equals the array's length and each rule matches the corresponding marked path in selection order. Use positional sequences for operations with distinct roles per argument, such as diffing two files or appending selected files to a selected archive.

Each slot in the array may be either a single rule string or an inner array of rules that must all match that slot.

### Scoped rule table

A scoped table `{ selected, condition, strict }` evaluates a rule set against a chosen part of the picker state:

```toml
condition = { selected = "active", condition = ["mime:image/*", "have:cwebp"] }
```

`condition` inside a scoped table defaults to `"*"` and accepts either a single rule string or an array of rules that must all match every inspected path. `selected` defaults to `"cursor"` and `strict` defaults to `false`:

| `selected`              | What it inspects                            | `strict = false`                            | `strict = true`                           |
| ----------------------- | ------------------------------------------- | ------------------------------------------- | ----------------------------------------- |
| `"cursor"`              | Highlighted row                             | Requires an active cursor row               | Also requires no marked selections        |
| `"cwd"`                 | Working directory                           | Requires the prompt focused on the pane cwd | Matches in any Nav pane against `nav_cwd` |
| *integer* `n`           | Marked selections                           | At least `n` marked paths                   | Exactly `n` marked paths                  |
| `"active"` (`"single"`) | Marked paths, else cursor, else focused cwd | Any number of resolved targets              | Exactly one resolved target               |

Using `selected = "cwd"` also redirects `paths`: non-strict `cwd` passes the pane's working directory as a one-element `paths` table, and `strict = true` passes `nav_cwd`. Similarly, `selected = "active"` passes the pane directory when nothing is marked and the prompt is focused.

### Rule syntax

Menu conditions reuse `lessfilter.toml` pattern rules without score modifiers:

| Rule                                        | Matches                                                                                          |
| ------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| `*`                                         | Any path                                                                                         |
| `ext:<ext>` or `.<ext>`                     | Case-insensitive extension (`ext:` matches extensionless paths)                                  |
| `glob:<pattern>`                            | Full path matching the glob                                                                      |
| `child:<pattern>`                           | Directory containing an entry matching the glob (or sibling for a file)                          |
| `mime:<type>/<sub...>` or `<type>/<sub...>` | MIME type (`*` wildcards supported, such as `image/*`)                                           |
| `cat:<name>` or `category:<name>`           | Built-in category or custom `[categories]` group (`fs :tool types`)                              |
| `type:<ft>`                                 | `f` (file), `d` (directory), `l` (symlink), `x` (executable), `b`, `c`, `s`, `p`, `e`, or `text` |
| `application` or `app`                      | Platform application (`.app`, `.desktop`/`.AppImage`, or `.exe`)                                 |
| `git`                                       | Path inside a git work tree                                                                      |
| `have:<program>`                            | `<program>` is available on `PATH`                                                               |

Prefix any rule with `!` to negate it (`!type:d`, `!have:rg`).

## Examples

Ready-to-use action bundles for archiving, diffing, media conversion, file organization, and sharing ship in [`assets/actions/`](https://github.com/Squirreljetpack/fist/tree/main/assets/actions).

### Diff two selected files in the pager (`ExecPaged`)

Using a positional sequence ensures the action only appears when two regular files are marked, then pipes `diff -u` (falling back to `git diff --no-index`) into the pager:

```toml
["diff"]
prefix = ""
strategy = "ExecPaged"
condition = ["type:f", "type:f"]
command = """
local p1 = util.shq(paths[1])
local p2 = util.shq(paths[2])
local ok, _, code = os.execute("diff -u --color=always " .. p1 .. " " .. p2 .. " 2>/dev/null")
if not ok and code ~= 1 then
  local g_ok = os.execute("git diff --no-index --color=always " .. p1 .. " " .. p2)
  if g_ok then
    print(lunacolors.bold("Files are identical:") .. "\\n  " .. paths[1] .. "\\n  " .. paths[2])
  end
elseif ok then
  print(lunacolors.bold("Files are identical:") .. "\\n  " .. paths[1] .. "\\n  " .. paths[2])
end
"""
```

### Create or append to a zip archive (`Exec`)

When the first marked path is already a `.zip`, the remaining targets are appended to it; otherwise a new archive is created next to the first target (or inside `to` when `--to` is passed on the CLI). Gating on `have:zip` hides the item if `zip` is not installed:

```toml
["zip"]
strategy = "Exec"
help = "Select an existing .zip first to append to it; otherwise creates a new archive."
condition = { selected = "active", condition = "have:zip" }
command = """
local is_archive = paths[1]:lower():match("%.zip$") ~= nil
local archive = is_archive and paths[1] or path.join(to ~= "" and to or path.parent(paths[1]), path.base(paths[1]) .. ".zip")
if is_archive and path.is_empty(archive) then os.remove(archive) end
local first_idx = is_archive and 2 or 1
local names = {}
for i = first_idx, #paths do
  names[#names + 1] = util.shq(paths[i])
end
if #names == 0 then error("no files to add to " .. path.base(archive)) end
local ok, _, code = os.execute("zip -rq -j " .. util.shq(archive) .. " " .. table.concat(names, " "))
if not ok then error("zip failed: " .. tostring(code)) end
toast_push("success", is_archive and "Updated" or "Compressed", path.base(archive))
"""
```

### Toggle executable bit (`ExecSilent`)

Running quietly in the background, this checks `fs.stat(p).executable` across all active targets and toggles `+x` or `-x`:

```toml
["chmod +x"]
prefix = ""
condition = { selected = "active" }
command = """
local all_exec = true
for _, p in ipairs(paths) do
  local st = fs.stat(p)
  if not (st and st.executable) then
    all_exec = false
    break
  end
end
local mode = all_exec and "-x" or "+x"
for _, p in ipairs(paths) do
  if not fs.chmod(p, mode) then
    error("chmod " .. mode .. " failed on " .. p)
  end
end
local item = #paths == 1 and path.base(paths[1]) or (#paths .. " items")
toast_push("success", "set " .. mode, item)
"""
```

### Interactive prompt (`Exec` and `input`)

You can prompt with `input.prompt`, `input.confirm`, or `input.select`, or launch `$EDITOR` via `proc.editor`:

```toml
["tailscale"]
prefix = "share"
strategy = "Exec"
condition = { selected = "active", condition = ["!type:d", "have:tailscale"] }
command = """
local host = input.prompt("Send to tailscale host:")
if not host or host == "" then return end
local args = { "file", "cp" }
for _, target in ipairs(paths) do args[#args + 1] = target end
args[#args + 1] = host .. ":"
local res = proc.run("tailscale", args)
if res.ok then
  toast_push("success", "Sent to " .. host, #paths == 1 and path.base(paths[1]) or (#paths .. " items"))
else
  toast("error", "tailscale file cp: " .. res.stderr)
end
"""
```

### Directory-scoped command (`selected = "cwd"`)

Pairing `selected = "cwd"` with `strict = true` keeps a repository tool available anywhere inside a Nav pane within a git tree and passes that Nav directory in `paths[1]`:

```toml
["lazygit"]
prefix = ""
strategy = "Exec"
condition = { selected = "cwd", condition = ["git", "have:lazygit"], strict = true }
command = """
local res = proc.run("lazygit", {}, { cwd = paths[1] })
if not res.ok then error("lazygit exited with " .. tostring(res.code)) end
"""
```

### Queued action requiring a destination (`Queue` and `requires_dest`)

Setting `strategy = "Queue"` and `requires_dest = true` lets you mark files across directories, edit the `To` column in the queue overlay (`alt-u`), and report progress through `set_progress`:

```toml
["convert webp"]
prefix = "image"
strategy = "Queue"
requires_dest = true
condition = { selected = "active", condition = ["mime:image/*", "have:cwebp"] }
command = """
if not path.is_dir(to) then error("destination is not a directory: " .. to) end
for i, p in ipairs(paths) do
  set_progress(math.floor((i - 1) / #paths * 255))
  local out_file = path.join(to, path.stem(p) .. ".webp")
  local res = proc.run("cwebp", {"-q", "85", p, "-o", out_file})
  if not res.ok then
    toast_push("error", "Failed", path.base(p))
  end
end
set_progress(255)
"""
```

## Running from the shell

You can run any menu action directly from the command line without opening the TUI:

```shell
fs :tool action "image: to png" --to ~/out photo.jpg
fs :tool action flatten ~/Downloads/unsorted
```

Pass either the menu label with its prefix (`"image: to png"`) or the bare key (`"to png"`). Options precede the target paths, `--` separates paths beginning with `-`, and omitting paths defaults to the current working directory.

`--to <dir>` supplies `to` and must point to an existing directory; it is required whenever `requires_dest = true`. Toasts print to standard error, `input` and `proc.editor` are available only for `Exec` and `ExecWait`, `set_progress` is defined for queued strategies, and calling `os.exit(code)` sets the process exit status.

## Queue interaction

Menu actions using `Queue`, `{ QueueBatch = n }`, or `Enqueue` integrate directly with the queue under their action key:

- **Keybindings in `mm.toml`**: Bind `Enqueue("<key>")` to queue targets directly without opening the menu, `ExecuteQueue("<key>")` to dispatch pending rows of that kind, and `ClearQueue("<key>")` to clear them (see [actions.md](actions.md)). Action keys in queue bindings are case-sensitive.
- **Destinations**: Queued menu rows do not auto-fill `to` from the Nav pane's working directory. When `requires_dest = true`, edit the row's `To` cell in the queue overlay (`f2` or `ctrl-shift-r`) before dispatching; `ExecuteQueue(all)` skips rows missing a required destination, while `ExecuteQueue(<key>)` refuses until `To` is set.
- **Filtering**: Inside the queue overlay (`alt-u`), press `ctrl-z` or `alt-z` to cycle the `[kind: ...]` filter to rows of a specific menu action.
