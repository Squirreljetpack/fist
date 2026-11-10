# Lua Scripting & Runtime Modules

Fist embeds an isolated Lua 5.4 VM with bundled standard modules, terminal styling, and filesystem utilities. Lua scripts power two primary features in Fist:

1. **[Menu & Queue Actions](actions.md)** — custom commands, converters, archivers, and tools executed against selected files or directories from the menu overlay (`ctrl-e`/`alt-e`).
2. **[Custom Stream Transforms](#custom-stream-transforms-fs-custom)** — dynamic line filtering, path formatting, and context annotations in `fs :custom --transform` and `fs -u --transform`.

---

## Built-in Modules

Every Lua execution context has access to pre-injected standard helper modules. They are accessible directly as globals (e.g. `path.join(...)`) or via `require("<name>")` (e.g. `local fs = require("fs")`):

### 1. `path` Module

Path manipulation, normalization, and query functions:

| Function                 | Return Type     | Description                                                                                       |
| ------------------------ | --------------- | ------------------------------------------------------------------------------------------------- |
| `path.parent(p)`         | `string`        | Directory parent (`/` for root, `.` for relative basename).                                       |
| `path.base(p)`           | `string`        | Trailing file or directory name component.                                                        |
| `path.stem(p)`           | `string`        | File name stripped of final extension (e.g. `foo.tar.gz` -> `foo.tar`).                           |
| `path.ext(p)`            | `string`        | File extension without leading dot (e.g. `"gz"`, or `""` if none).                                |
| `path.join(...)`         | `string`        | Joins multiple path components together.                                                          |
| `path.exists(p)`         | `bool`          | Tests whether `p` exists on the filesystem (follows symlinks).                                    |
| `path.lexists(p)`        | `bool`          | Tests whether `p` exists without following symlinks (true for broken symlinks).                   |
| `path.is_file(p)`        | `bool`          | Tests if `p` exists and is a regular file.                                                        |
| `path.is_dir(p)`         | `bool`          | Tests if `p` exists and is a directory.                                                           |
| `path.is_empty(p)`       | `bool`          | Tests if a file is 0 bytes or a directory contains 0 entries.                                     |
| `path.rel(target, base)` | `string \| nil` | Relative path from `base` to `target`; `nil` if only `base` is absolute or contains `..`.         |
| `path.mime(p)`           | `string`        | Inspects file header/extension and returns MIME type string (e.g. `"image/png"`, `"text/plain"`). |

---

### 2. `fs` Module

Native filesystem and metadata operations:

| Function                          | Return Type    | Description                                                                                             |
| --------------------------------- | -------------- | ------------------------------------------------------------------------------------------------------- |
| `fs.stat(p)`                      | `table \| nil` | Returns table with `{ size, is_file, is_dir, is_symlink, readonly, mtime, mode, executable }`.          |
| `fs.chmod(p, mode)`               | `bool`         | Modifies file permissions (`mode` accepts number like `0o755`, octal string `"755"`, or `"+x"`/`"-x"`). |
| `fs.mkdir(p, [recursive=true])`   | `bool`         | Creates directory.                                                                                      |
| `fs.readdir(p)`                   | `table`        | Returns array of filename strings in directory `p`.                                                     |
| `fs.read(p)`                      | `string`       | Reads entire file into a string.                                                                        |
| `fs.write(p, content)`            | `bool`         | Writes string content to a file.                                                                        |
| `fs.remove(p, [recursive=false])` | `bool`         | Deletes file or directory.                                                                              |
| `fs.copy(src, dst)`               | `bool`         | Copies file from `src` to `dst`.                                                                        |
| `fs.rename(src, dst)`             | `bool`         | Moves/renames `src` to `dst`.                                                                           |
| `fs.symlink(src, dst)`            | `bool`         | Creates a symbolic link at `dst` pointing to `src`.                                                     |
| `fs.readlink(p)`                  | `string`       | Reads the target path of a symbolic link.                                                               |
| `fs.canonicalize(p)`              | `string`       | Resolves all symlinks and relative components to an absolute path.                                      |
| `fs.trash(path_or_paths)`         | `bool`         | Moves file(s) or directories to system trash. Accepts string or array of strings.                       |
| `fs.tempdir([prefix])`            | `string`       | Creates and returns a unique temporary directory path.                                                  |
| `fs.tempfile([prefix])`           | `string`       | Creates and returns a unique temporary empty file path.                                                 |

```lua
local st = fs.stat(p)
if st and not st.executable then
  fs.chmod(p, "+x")
end
```

---

### 3. `proc` Module

Subprocess execution without shell interpolation:

| Function                           | Return Type | Description                                                                                                                                                                                      |
| ---------------------------------- | ----------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `proc.run(prog, [args], [opts])`   | `table`     | Runs command and waits for output. Returns `{ ok = bool, code = number, stdout = string, stderr = string }`.                                                                                     |
| `proc.spawn(prog, [args], [opts])` | `table`     | Spawns background process without waiting. Returns `{ ok = bool, pid = number }`.                                                                                                                |
| `proc.has(prog)`                   | `bool`      | Fast in-process check if `prog` exists on `PATH`.                                                                                                                                                |
| `proc.editor([path])`              | `table`     | Interactive only. Runs `$FS_EDITOR` (then `$EDITOR`, then `nano`) in the foreground with the terminal attached, waiting for it to exit. Returns `{ ok = bool, code = number, editor = string }`. |

**Options table (`opts`)**:

- `cwd` (string): Working directory for the child process.
- `env` (table): Key-value map of environment variables.
- `stdin` (string): Data to pipe into child process standard input.

```lua
if proc.has("ffmpeg") then
  local res = proc.run("ffmpeg", {"-y", "-i", input_file, out_file}, { cwd = "/tmp" })
  if not res.ok then
    error("ffmpeg error (" .. res.code .. "): " .. res.stderr)
  end
end
```

---

### 4. `util` Module

Utility functions for serialization, time, clipboard, and shell quoting:

| Function                           | Return Type     | Description                                                                                                                                 |
| ---------------------------------- | --------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| `util.sleep(ms)`                   | `nil`           | Pauses execution for `ms` milliseconds.                                                                                                     |
| `util.uuid()`                      | `string`        | Generates a random v4 UUID string.                                                                                                          |
| `util.timestamp([fmt])`            | `string`        | Current time. Default: ISO 8601 string. `"unix"`/`"epoch"`: seconds string. `"ms"`: milliseconds. Or custom strftime format (`"%Y-%m-%d"`). |
| `util.json_encode(value)`          | `string`        | Serializes a Lua value/table into a JSON string.                                                                                            |
| `util.json_decode(text)`           | `value`         | Parses a JSON string into Lua values/tables.                                                                                                |
| `util.toml_encode(table)`          | `string`        | Serializes a Lua key-value table into a TOML string.                                                                                        |
| `util.toml_decode(text)`           | `value`         | Parses a TOML string into Lua values/tables.                                                                                                |
| `util.clipboard_get()`             | `string \| nil` | Reads text from system clipboard (integrates with terminal OSC 52).                                                                         |
| `util.clipboard_set(text, [opts])` | `nil`           | Writes text to clipboard. Accepts optional `{ toast = false }` table.                                                                       |
| `util.shq(str)`                    | `string`        | Quotes and escapes a string for POSIX shell commands.                                                                                       |

---

### 5. `input` Module *(Interactive Only)*

> [!NOTE]
> Active only in interactive foreground actions (`Exec` and `ExecWait`). In background or queued executions, `input` is `nil` to prevent workers from blocking.

| Function                        | Return Type     | Description                                                                               |
| ------------------------------- | --------------- | ----------------------------------------------------------------------------------------- |
| `input.prompt(msg, [default])`  | `string \| nil` | Prompts user for a line of text via `/dev/tty`. Returns string or `default`.              |
| `input.confirm(msg, [default])` | `bool`          | Displays `[Y/n]` or `[y/N]` prompt. Returns boolean `true`/`false`.                       |
| `input.select(items, [prompt])` | `string \| nil` | Interactive fuzzy picker via Fist's matchmaker. Returns selected item or `nil` on cancel. |

```lua
local choice = input.select({"webp", "png", "avif"}, "Select format:")
if not choice then return end

local confirmed = input.confirm("Overwrite existing files?", false)
```

---

### 6. `lunacolors` Module

Terminal ANSI styling and formatting:

```lua
-- Directly styled functions:
print(lunacolors.bold("Important Header"))
print(lunacolors.green("Success: ") .. file)
print(lunacolors.red(lunacolors.underline("Error!")))

-- Template formatting with automatic reset:
print(lunacolors.format("{bold}{yellow}Warning:{reset} file already exists"))
```

Available colors: `black`, `red`, `green`, `yellow`, `blue`, `magenta`, `cyan`, `white`, `brightBlack` (`gray`), `brightRed`, `brightGreen`, `brightYellow`, `brightBlue`, `brightMagenta`, `brightCyan`, `brightWhite`. Background variants are suffixed with `Bg` (e.g. `redBg`).
Available styles: `bold`, `dim`, `italic`, `underline`, `invert`, `hidden`, `strikethrough`.

---

## Global Functions & Action Environment

When executing [Menu Actions](actions.md), scripts also receive context arguments and lifecycle globals:

### Arguments & Globals

- `paths` (table) — array of targeted absolute paths.
- `dst` (string) — destination directory (empty for direct menu runs; populated for queued actions).
- `nav_cwd` (string) — active Nav pane directory.

### Host Integration Functions

- `toast(style, msg)` — displays a footer notice toast (`"info"`, `"success"`, `"warning"`, `"error"`, or `"normal"`).
- `toast_push(style, prefix, item)` — adds a bulleted item to a grouped toast summary.
- `set_progress(0-255)` — updates the queue row's progress bar (0–255 scale). Defined only for queued actions (`Queue`, `QueueBatch`, `Enqueue`).
- `os.exit(code)` — safely halts script execution with the given exit code without exiting Fist.
- `error(...)` — stops execution with a runtime error toast.

---

## Custom Stream Transforms (`fs :custom`)

The custom pane (`fs :custom`) enables fuzzy navigation and preview over arbitrary Unix streams or commands. Passing `--transform` lets you dynamically rewrite paths, format display titles, and populate secondary context columns.

### Invocation

```bash
# Transform pipeline output:
find . -name "*.md" | fs :custom --transform 'local p = ... return p, p:match("[^/]+$")'

# Transform command output with external script:
fs :custom --tail-sep $'\t' --transform @transform.lua -- my-generator-cmd
```

### Transform Function Contract

The transform chunk is invoked per line with positional arguments passed to `...`:

```lua
local path, tail = ...
```

- `path` (string): The resolved path (or token before `--tail-sep`).
- `tail` (string): Context string after `--tail-sep` (or empty string).

#### Return Values

Your transform chunk must return up to three values:

```lua
return path, display, tail
```

1. **`path`** *(string or nil)*: The filesystem path used for file preview and opening. Returning `nil` **filters out** the line completely.
2. **`display`** *(string or nil)*: The string displayed in the primary column. If `nil`, falls back to `path`.
3. **`tail`** *(string or nil)*: Context badge rendered in the secondary column.

### Transform Examples

#### 1. Vault Slicing & Markdown Formatting

```bash
fs -t .md --list --format '{=}\t{-1.}' . | \
  fs :custom --opener ob-open \
    --tail-sep $'\t' \
    --transform '
local path, tail = ...
local display = (tail and tail ~= "") and ("/" .. path):match("^.-/" .. tail:gsub("%W", "%%%1") .. "/(.*)") or path
return path, display:gsub("^/+", ""):gsub("%.md$", ""), tail
'
```

#### 2. Line Filtering

Excludes `.git` and `.tmp` files by returning `nil`:

```bash
find . -type f | fs :custom --transform '
local path = ...
if path:match("/%.git/") or path:match("%.tmp$") then
    return nil -- drops the row
end
return path
'
```

#### 3. Mapping `.git` Folders to Project Roots

```bash
fs -u --transform '
local path, tail = ...
local parent = path:gsub("/%.git$", "")
if parent ~= path then
    return parent, parent:match("[^/]+$")
end
return path, path
' --sort mtime ~/gh ~/projects '\\.git$'
```

#### 4. External Transform Script (`@file`)

```lua
-- ~/.config/fist/transforms/git_status.lua
local line = ...
local status, file = line:match("^%s*(%S+)%s+(.+)$")
if not status then return line end

return file, file, "[" .. status .. "]"
```

Run with:

```bash
git status --porcelain | fs :custom --transform @~/.config/fist/transforms/git_status.lua
```
