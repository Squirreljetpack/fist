# Menu Actions

Menu actions are user-defined commands shown in the menu overlay (`ctrl-e`/`alt-e`).
They are declared as TOML tables keyed by name; the insertion order is the
menu display order.

Actions live in `actions.toml` next to `config.toml`, plus every `*.toml` file
in the `actions/` folder next to it, merged in sorted filename order (numeric
prefixes order them). `--dump-config` writes the shipped `actions.toml`.

The action key serves two roles:

- the **queue kind**: `ExecuteQueue(kind)` / `ClearQueue(kind)` select queue
  rows by the exact key,
- the second half of the **name** shown in the menu, `{prefix}: {key}` (see
  `prefix` below).

The following keys are reserved and rejected (case-insensitively): the
builtin queue kinds `copy`, `move`, `symlink`, `none`, the selectors `all`,
`builtins`, `first`, `last`, and the empty key.

## Fields

| Field           | Type           | Default          | Meaning                                            |
| --------------- | -------------- | ---------------- | -------------------------------------------------- |
| `command`       | string         | required         | The lua script (`@file` syntax supported).         |
| `prefix`        | string         | file stem        | Menu group label; `""` drops it. See below.        |
| `alias`         | string         | none             | The alias shown in the menu's second column.       |
| `help`          | string         | `""`             | Description of the action; not rendered yet.       |
| `strategy`      | string         | `"ExecSilent"`   | How the command runs; see below.                   |
| `condition`     | list or object | always visible   | Visibility rules; see below.                       |
| `requires_dest` | bool           | `false`          | Queued executions require a non-empty destination. |
| `close`         | bool           | strategy default | Override the strategy's menu-closing behavior.     |

### Prefixes

The menu row reads `{prefix}: {key}`, so `["merge"]` with `prefix = "pdf"`
shows as `pdf: merge`. The default value `:` is replaced at load time with the
stem of the file that declared the action (`actions/pdf.toml` → `pdf`), which
groups actions by file. An empty prefix renders the key alone. Two actions may
not render the same row; `fs :tool check` reports such pairs as errors.

`@file` commands load the script from disk (`~/` and absolute paths work; relative paths resolve against the actions folder).

## Strategies

| Strategy     | Waits | Closes menu | Effect                                                  |
| ------------ | ----- | ----------- | ------------------------------------------------------- |
| `Exec`       | yes   | yes         | Run the lua command and wait for it.                    |
| `ExecWait`   | yes   | yes         | Run the lua command, wait for it, and prompt for Enter. |
| `ExecSilent` | no    | yes         | Run in background.                                      |
| `ExecPaged`  | yes   | yes         | Run and page its stdout.                                |
| `Queue`      | —     | no          | Enqueue all targets as one multi-path queue item.       |
| `QueueBatch` | —     | no          | Enqueue the targets in chunks of at most *n* paths.     |
| `Enqueue`    | —     | yes         | Enqueue all targets as one queue item and start it.     |

`Queue`/`QueueBatch`/`Enqueue` create queue rows of the action's key; the row's
destination is editable in the queue overlay (`alt-u`). `requires_dest` cannot
be combined with `Enqueue`, whose row starts immediately with no destination.

## The Lua Environment & Global Contract

Commands run inside an isolated Lua 5.4 VM per execution. Arguments are provided both as function arguments `(...)` and directly as globals.

For the exhaustive API reference and module documentation, see [Lua Scripting & Runtime Modules](lua.md).

### Provided Globals & Arguments

- `paths` (table) — array of targeted absolute paths as strings.
- `dst` (string) — the destination string (empty for direct menu calls; the queue row's destination for queued runs).
- `nav_cwd` (string) — the Nav pane directory (injected if present).

### Built-in Global Functions

- `toast(style, msg)` — displays a footer notice toast. `style` accepts `"info"`, `"success"`, `"warning"` (`"warn"`), `"error"` (`"err"`), or `"normal"` (`nil`).
- `toast_push(style, prefix, item)` — appends an item to a grouped list with a styled prefix (e.g. `toast_push("success", "set +x: ", "script.sh")` or `toast_push("success", "Compressed: ", "archive.zip")`).
- `set_progress(0-255)` — updates the executing queue item's progress bar (0–255 scale). Defined only for queued actions (`Queue`, `QueueBatch`, `Enqueue`).
- `os.exit(code)` — safely stops script execution with the given exit code without terminating the host `fs` process.
- `error(...)` — raises a runtime error, stopping execution and displaying a failure notice toast.

Every action also has access to pre-injected standard helper modules, see [Lua Scripting & Runtime Modules](lua.md).

## Conditions

An action is visible iff at least one condition passes; an empty list means
always visible. A single condition is accepted without the outer array wrapper.
Conditions are evaluated once when the menu opens.

Alternatives that can never pass because they require a program that is not
installed (`have:qpdf`) are removed while loading; an action whose alternatives
are all removed is dropped, so `:tool check` (which loads unfiltered) is the
only place those rows appear. Omission is the only way to spell an
unconditional action: a written list always holds at least one alternative.

There are two forms:

### 1. Positional Sequence (`["type:f", "type:d"]`)

Direct array of file rules evaluated positionally against selections in order:

```toml
condition = ["type:f", "type:d"]
```

Exactly as many items must be selected as there are rules, and rule *i*
must match the *i*-th selected item in selection order. Multiple sequence alternatives
can be supplied in an array (`condition = [["type:text", "type:text"], ["type:text", "type:d"]]`).

### 2. Scoped Rule Table (`{ selected = "...", condition = "...", strict = bool }`)

Flat table evaluated against the target set chosen by `selected`:

```toml
condition = { selected = "active", condition = "type:f", strict = true }
```

The rule must match **every** path in the target set:

| `selected`           | strict = false                                                                                                | strict = true                                     |
| -------------------- | ------------------------------------------------------------------------------------------------------------- | ------------------------------------------------- |
| `"cursor"` (default) | an enabled cursor with an item (selections are ignored)                                                       | additionally: nothing selected                    |
| `"cwd"`              | a current directory **while the cursor is disabled** (the prompt state)                                       | the Nav pane directory, regardless of the cursor  |
| *n*                  | at least *n* selected items                                                                                   | exactly *n* selected items                        |
| `"active"`           | the selection when any items are selected, else the cursor item, else (cursor disabled) the current directory | the resolved target set contains exactly one path |

For `"cursor"` the rule is evaluated against the cursor item; for `"cwd"`
against the current directory (strict: the Nav directory); for *n* and
`"active"` against every resolved path. `strict` defaults to `false`;
`selected` defaults to `"cursor"` and `condition` to `*` (any target) when
omitted, so `{ selected = "active" }` shows the action for whatever is
targeted.

### Rule syntax

| Rule                                                    | Matches                                                             |
| ------------------------------------------------------- | ------------------------------------------------------------------- |
| `ext:rs`                                                | File extension (`rs`). Bare `.ext` is shorthand (`.rs`).            |
| `glob:*.rs`                                             | The full path against the glob.                                     |
| `child:src`                                             | A child of the directory (or of a file's parent) matching the glob. |
| `mime:image/*`                                          | MIME type (type may be a wildcard; e.g. `mime:image/png`).          |
| `type:f` / `type:d` / `type:l` / `type:x` / `type:text` | File / directory / symlink / executable / text file.                |
| `have:program`                                          | The program exists on PATH.                                         |
| `cat:name`                                              | A file category (builtin or user-defined).                          |
| `application`                                           | Platform app bundle / launcher / executable.                        |
| `git`                                                   | The path is inside a git work tree.                                 |
| `*`                                                     | Anything.                                                           |

Prefix with `!` to invert (`!ext:rs`).

## Examples

To view or use the ready-to-use action plugins shipped with fist (including compression, directory/file diffing, and image/video conversion), see [assets/actions](https://github.com/Squirreljetpack/fist/tree/main/assets/actions).

### 1. Direct Execution on Active Targets (`ExecSilent` + `fs`)

Runs in background. Toggles executability on targeted paths using the native `fs` module:

```toml
["chmod +x"]
strategy = "ExecSilent"
condition = { selected = "active", condition = "*" }
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
  fs.chmod(p, mode)
end
toast_push("success", "set " .. mode .. ": ", #paths == 1 and path.base(paths[1]) or (#paths .. " items"))
"""
```

### 2. Interactive Input & Prompting (`Exec` + `input`)

Prompts the user interactively before performing an action:

```toml
["rename-timestamp"]
strategy = "Exec"
condition = { selected = "active", condition = "type:f" }
command = """
local prefix = input.prompt("Enter filename prefix:", path.stem(paths[1]))
if not prefix or prefix == "" then return end

local new_name = prefix .. "_" .. util.timestamp("epoch") .. "." .. path.ext(paths[1])
local target = path.join(path.parent(paths[1]), new_name)

fs.rename(paths[1], target)
toast("success", "Renamed to " .. new_name)
"""
```

### 3. Styled Paged Output on Positional Pair (`ExecPaged` + `lunacolors`)

Requires exactly two files selected in order; runs diff and styles headers:

```toml
["diff"]
strategy = "ExecPaged"
condition = ["type:f", "type:f"]
command = """
local p1, p2 = paths[1], paths[2]
print(lunacolors.bold(lunacolors.underline(path.base(p1) .. " <-> " .. path.base(p2))))
os.execute("diff -u --color=always " .. util.shq(p1) .. " " .. util.shq(p2))
"""
```

### 4. Prompt-Scoped Command (`selected = "cwd"`)

Visible only when in the prompt state (cursor disabled) inside a git repo:

```toml
["git: log"]
strategy = "ExecPaged"
condition = { selected = "cwd", condition = "git" }
command = "os.execute('cd ' .. util.shq(paths[1]) .. ' && git log')"
```

### 5. Gated on External Tool (`{ condition = "have:..." }`)

Visible only when `tar` is installed on `PATH`:

```toml
["tar.gz"]
strategy = "Exec"
condition = { condition = "have:tar" }
command = "print('compressing ' .. #paths .. ' items to ' .. dst)"
```

### 6. Queued Action Requiring Destination (`Queue` + `requires_dest`)

Enqueues matching targets as a custom queue item; requires destination to execute:

```toml
["stash-logs"]
strategy = "Queue"
condition = { selected = "active", condition = "ext:log" }
requires_dest = true
command = "print('processing log ' .. paths[1] .. ' -> ' .. dst)"
```

## Running an Action from the Shell

```
fs :tool action <name> [--dest <dir>] [--] [paths...]
```

`<name>` is the menu label including its prefix (`image: to png`, `flatten`);
a key without its prefix is accepted too. Paths default to the current
directory. Actions with `requires_dest = true` demand `--dest`, and the
directory must exist; other actions accept it as well and receive it as `dst`.
Options must precede the paths; use `--` before a path that would otherwise
be parsed as an option. Both globals are set exactly as in a menu run —
`nav_cwd` is the current directory — and `toast`/`toast_push` print to stderr
instead of the overlay. Interactive strategies (`Exec`, `ExecWait`) can prompt
on the terminal; `Queue`/`QueueBatch` run the script once with every path,
like an immediate row. The exit code is the script's `os.exit` status when it
exits that way, and non-zero when the script fails.

## Interaction with the Queue

- `Queue`/`QueueBatch` rows execute the action's lua command once per row
  with the row's stored `dst` as the second argument.
- `requires_dest = true` rows need a non-empty destination: `All`
  (`ExecuteQueue` with no argument) silently skips such rows with an empty
  destination, exact selectors report an error.
- The queue overlay's `[kind: x]` filter cycles the shared queue by kind;
  `Paste` executes all pending `copy`, `move`, and `symlink` rows.
