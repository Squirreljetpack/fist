---
name: fist-menu-actions
description: Defines F:ist menu actions - context-aware Lua plugins shown in fs's alt-e menu and reusable as queueable bulk operations. Use when adding a custom action to the fs menu, gating visibility on file properties or installed programs, wiring queued multi-step operations with editable destinations, or debugging an action's strategy, conditions, or script.
compatibility: Requires the F:ist CLI (`fs`) with an initialized config directory (run `fs --dump-config` once). Action commands are Lua scripts executed by fs's embedded VM; no system Lua is required.
---

# F:ist menu action authoring

Use this skill to write, review, or debug a **menu action**: a named TOML entry whose Lua command runs against the items under the cursor or the current selection. Menu actions appear in the menu overlay (`alt-e`), can be bound to keys directly, and can feed the background queue for batch execution with per-row destinations.

## Source of truth

Before inventing fields or syntax, consult the version of F:ist being used:

```sh
fs --dump-config            # writes the default actions.toml next to config.toml on first run
fs :tool check              # parses every config file and compiles each action's lua (still maturing)
```

The shipped examples are the best style references: `compress.toml` (queued shell-outs gated on `have:`) and `diff.toml` (paged positional-pair comparisons) in the repository's `assets/actions/`, plus the `chmod +x` action in the default `actions.toml`.

If the request is ambiguous, clarify which targets the action should operate on (selection vs cursor vs cwd), whether it should run immediately or enqueue, and what its output should do (toast, page, mutate files) before writing it.

## Where actions live

- `actions.toml` next to `config.toml` (default: `~/.config/fist/actions.toml`) is the primary file.
- Every `*.toml` in the `actions/` folder next to it is merged recursively: innermost subfolders first, then sorted path order, so numeric prefixes order them.
- Duplicate keys across files are an error. The primary file's entries come first.

The action key has two roles:

1. The **name** displayed in the menu overlay.
2. The **queue kind** selected by `ExecuteQueue(kind)` / `ClearQueue(kind)` binds.

These keys are reserved (case-insensitive) and rejected: `copy`, `move`, `symlink`, `none`, `all`, `builtins`, `first`, `last`, `default`, and the empty key.

## Action model

A minimal action is a table with a `command`. Insertion order is menu display order.

| Field           | Type           | Default           | Meaning                                               |
| --------------- | -------------- | ----------------- | ----------------------------------------------------- |
| `command`       | string         | required          | Lua script (`@file` reference supported).             |
| `alias`         | string         | none              | Letter shown/typed in the menu to trigger the item.   |
| `strategy`      | string         | `"ExecuteSilent"` | How the command runs; see below.                      |
| `condition`     | list or object | always visible    | Visibility rules; see below.                          |
| `requires_dest` | bool           | `false`           | Queued executions require a non-empty destination.    |
| `close`         | bool           | strategy default  | Override whether choosing the action closes the menu. |

### Strategies

| Strategy             | Waits | Closes menu | Effect                                            |
| -------------------- | ----- | ----------- | ------------------------------------------------- |
| `Execute`            | yes   | yes         | Run the Lua command and wait.                     |
| `ExecuteSilent`      | no    | yes         | Run without waiting (alias: `silent`).            |
| `ExecPaged`          | yes   | yes         | Run, wait, and page stdout in the built-in pager. |
| `Queue`              | —     | no          | Enqueue all targets as one multi-path queue item. |
| `{ QueueBatch = n }` | —     | no          | Enqueue targets in chunks of at most *n* paths.   |

(`QueueBatch` carries a payload, so it is written as an inline table rather than a plain string.)

Prefer `ExecPaged` for read-only reporting (`git log`, diffs, listings), `ExecuteSilent` for fire-and-forget mutations with a toast, and `Queue`/`QueueBatch` when destinations matter or the work is long-running. `close = false` keeps the menu open after Execute-style actions for back-to-back use.

## The Lua environment

Each execution gets an isolated VM. Arguments arrive both as varargs and globals:

- `paths` (table of strings) — targeted absolute paths.
- `dst` (string) — empty for direct menu calls; the queue row's destination when run from the queue.
- `nav_cwd` (string) — the Nav pane directory, injected when present.

Built-in functions:

- `toast(style, msg)` — footer notice; styles `info`, `success`, `warning`/`warn`, `error`/`err`, `normal`/nil.
- `toast_push(style, prefix, item)` — grouped list toast (e.g. one row per processed file).
- `set_progress(0-255)` — the executing queue item's progress bar; no-op outside queue runs.
- `os.exit(code)` — stop the script safely (does not terminate `fs`).
- `error(...)` — abort and surface a failure toast.

`@file` commands load the script from disk: `~/` and absolute paths work; relative paths resolve against the actions folder. Keep non-trivial logic in sibling `.lua` files rather than giant inline strings.

Scripts run in the **process cwd**, not the targets' directory — `cd` explicitly:

```lua
os.execute('cd "' .. paths[1] .. '" && git log')
```

Always shell-quote interpolated values (the shipped actions define a local `shq` helper for this); never splice raw paths into command lines.

## Conditions

An action is visible iff **at least one** condition passes; an empty list means always visible. Conditions are evaluated once when the menu opens. A single condition may be written without the outer array.

### Positional sequences

```toml
condition = ["type:f", "type:d"]
```

Exactly as many items must be selected as there are rules, and rule *i* must match the *i*-th selected item. Alternatives are supplied as nested arrays:

```toml
condition = [["type:text", "type:text"], ["type:d", "type:d"]]
```

### Scoped rule tables

```toml
condition = { selected = "active", condition = "type:f", strict = true }
```

The rule must match every path in the target set chosen by `selected`:

| `selected`           | non-strict                                                         | strict                                      |
| -------------------- | ------------------------------------------------------------------ | ------------------------------------------- |
| `"cursor"` (default) | an enabled cursor with an item                                     | additionally: nothing selected              |
| `"cwd"`              | the prompt directory while the cursor is disabled                  | the Nav pane directory regardless of cursor |
| *n*                  | at least *n* selected                                              | exactly *n* selected                        |
| `"active"`           | selections, else cursor item, else prompt directory; fails if none | resolved target set is exactly one path     |

(`"single"` is accepted as a spelling of `"active"`.)

### Rule syntax

Same matcher as lessfilter rules:

| Rule                        | Matches                                                |
| --------------------------- | ------------------------------------------------------ |
| `ext:rs` / `.rs`            | File extension                                         |
| `glob:*.rs`                 | Full path against the glob                             |
| `child:src`                 | A child (or sibling, for files) matching the glob      |
| `mime:image/*`              | MIME type, wildcard type/subtype allowed               |
| `type:f` `d` `l` `x` `text` | File / dir / symlink / executable / text file          |
| `have:program`              | Program exists on PATH (use to gate tool dependencies) |
| `cat:name`                  | Built-in or configured file category                   |
| `application`               | Platform app bundle / launcher / executable            |
| `git`                       | Path inside a git work tree                            |
| `*`                         | Anything                                               |

Prefix `!` to invert (`!ext:log`).

## Interaction with the queue

- `Queue`/`QueueBatch` create rows keyed by the action name; each row's destination is editable in the queue overlay (`alt-u`, `To` column).
- On execution the script runs once **per row** with that row's stored `dst`; multi-path rows receive all their paths in `paths`.
- `requires_dest = true`: executing via `All` silently skips rows with empty destinations; exact selectors report an error instead.
- Binds compose with selectors: `Enqueue(my-action)`, `ExecuteQueue(my-action)`, `ClearQueue(my-action)`, `Paste` (= `ExecuteQueue(builtins)`).

## A practical authoring workflow

1. **Define the contract**: targets (selection/cursor/cwd), immediate vs queued, output (toast/paged/file changes), external tools.
2. **Gate visibility** with conditions so the menu stays contextual; gate tools with `have:` so entries hide when dependencies are missing.
3. **Pick the smallest strategy** that fits; reach for `Queue` only when destinations or batching genuinely help.
4. **Quote everything** interpolated into shell commands; keep diagnostics off paged stdout only when they belong there.
5. **Report results** with `toast_push` per item or a final `toast`.
6. **Exercise non-interactively**: `fs :tool check` catches parse/lua-compile errors (treat it as a lint, not a guarantee). Then run `fs`, select appropriate targets, and confirm the entry appears and behaves.
7. **Test edge cases**: nothing selected (cursor on a file/directory), the prompt state, many items, spaces and quotes in names, missing destination for queued rows, and re-running idempotently.

## Common failure modes

- **Action never shows:** the condition is too strict (e.g. `strict = true` with selections present under `cursor`), or a `have:` dependency is missing.
- **Action shows but errors:** the script assumed `dst` or `nav_cwd` exists — both may be empty strings.
- **Paths break on spaces/quotes:** values were concatenated without shell quoting.
- **Wrong working directory:** scripts run in the process cwd; `cd` to the target explicitly.
- **Duplicate key error at startup:** two action files define the same key; rename one.
- **Reserved-key rejection:** `copy`/`all`/etc. cannot be action names — pick another kind.
- **Long-running silent actions look frozen:** use `set_progress` and toasts, or make them queued rows so the overlay shows progress.
