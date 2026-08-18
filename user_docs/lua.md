# Custom pane

The custom pane (`fs :custom`) enables interactive fuzzy navigation and preview over arbitrary Unix streams, shell command outputs, or transformed file listings. Combined with Lua transforms via `--transform`, you can dynamically rewrite paths, modify display labels, populate context columns, or filter items on the fly.

## Overview & Invocation

`fs :custom` accepts input from standard input (via pipes) or directly runs a command:

```bash
# Read from a pipe:
find . -name "*.md" | fs :custom

# Run a command directly:
fs :custom git status --short

# Stream with delimiter splitting and Lua transformations:
fs :custom --tail-sep $'\t' --transform @transform.lua -- my-generator-cmd
```

## CLI Options

| Flag          | Short | Meaning                                                                                        |
| ------------- | ----- | ---------------------------------------------------------------------------------------------- |
| `[CMD]...`    | —     | Command to execute and stream stdout from. If omitted, reads stdin.                            |
| `--transform` | —     | Lua script inline or `@file.lua` path to transform/filter each row.                            |
| `--tail-sep`  | `-ts` | Delimiter character used to split each input line into `(path, tail)`.                         |
| `--input-sep` | `-is` | Record separator character (defaults to newline `\n`; e.g., `\0` for null-terminated streams). |
| `--sort`      | —     | Sort order applied to stream items (`mtime`, `ctime`, `atime`, `size`, `name`, `score`, etc.). |
| `--opener`    | —     | Program or script used to open the selected entry.                                             |
| `--cd`        | —     | Print first match to stdout and exit (useful for non-interactive scripting).                   |

---

## The Lua Transform Contract

The `--transform` argument accepts either an inline Lua snippet or an `@path/to/script.lua` file reference (relative paths resolve against the current working directory).

### Input Arguments

The script chunk is invoked per line. It receives positional arguments passed into `...`:

```lua
local path, tail = ...
```

- `path` (string): The resolved absolute path (or the initial token before `--tail-sep`).
- `tail` (string): The context remainder string after splitting on `--tail-sep` (or an empty string if `--tail-sep` was not specified or not found).

### Return Values

Your transform chunk must return up to three values:

```lua
return path, display, tail
```

1. **`path`** *(string or nil)*: The final filesystem path associated with the item (used for file operations, preview generation, and opening).
   > **Filtering**: If `path` is `nil` or a non-string value, the entry is **omitted** from the listing entirely.
2. **`display`** *(string or nil)*: The formatted string rendered in the primary column. If `nil`, fist falls back to the default display path.
3. **`tail`** *(string or nil)*: The string rendered in the secondary context column. If `nil`, fist preserves the original `tail` string from `--tail-sep`.

---

## Examples

### 1. Vault Slicing & Markdown Formatting

Strip file extensions and display Obsidian vault-relative paths alongside a context tag:

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

### 2. Filtering Items

Exclude temporary files and hidden directories by returning `nil`:

```bash
find . -type f | fs :custom --transform '
local path = ...
if path:match("/%.git/") or path:match("%.tmp$") then
    return nil -- drops the row
end
return path
'
```

### 3. Mapping `.git` Folders to Project Roots

Transform matches of `.git` directories so that selecting the row targets and displays the parent project folder:

```bash
fs -a --transform '
local path, tail = ...
local parent = path:gsub("/%.git$", "")
if parent ~= path then
    return parent, parent:match("[^/]+$")
end
return path, path
' --sort mtime ~/gh ~/projects '\\.git$'
```

### 4. External Script File (`@file`)

For complex formatting, maintain scripts in reusable `.lua` files:

```lua
-- ~/.config/fist/transforms/git_status.lua
local line = ...
local status, file = line:match("^%s*(%S+)%s+(.+)$")
if not status then return line end

-- display status badge in the context column and clean path in primary
return file, file, "[" .. status .. "]"
```

Invoke with `@`:

```bash
git status --porcelain | fs :custom --transform @~/.config/fist/transforms/git_status.lua
```
