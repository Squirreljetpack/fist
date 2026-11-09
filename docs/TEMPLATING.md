# Item templates

Item templates substitute `{…}` placeholders in strings that describe the
current item, the current selection, or the current directory. The same
grammar is used everywhere templates are expanded, so a placeholder learned in
one place works in another (the available data differs per context, see
[Where templates are expanded](#where-templates-are-expanded)).

## Grammar

```
{ body }
```

A placeholder reads the current item, or every selected item when the range
delimiter is `+`. A range `S<delim>E` slices path components, and the delimiter
alone picks the base, the operation and the quoting.

| Body          | Meaning                                                         |
| ------------- | --------------------------------------------------------------- |
| *(empty)*     | the item's path — the "line"; follows the context default       |
| `S:E`         | component range of the current path, **quoted**                 |
| `S=E`         | component range of the current path, **raw**                    |
| `S.E`         | component range of the **current directory**, **raw**           |
| `S~E`         | display truncation: first `S` components `…/` last `E`, **raw** |
| `S+E`         | component range of **every selected item**, context default     |
| `+t`, `+tail` | the tail column of **every selected item**, raw                 |
| `+#`          | the index of **every selected item**, raw                       |
| `t`           | the tail column (column 2), raw                                 |
| `tail`        | alias for `t`                                                   |
| `#`           | index of the item (0-based), raw                                |

Multi values are formatted one by one and joined with a single space, falling
back to the current item when nothing is selected.

`S` and `E` are optional signed integers and may both be empty (`{=}` is the
whole path raw, `{:}` the whole path quoted, `{+}` the whole path of every
selected item). Negative values count from the end, and an end of `0` (or
empty) means "to the end". `~` has no negative form: it keeps the head and tail
components and joins them with `…/`, printing the whole path when both sides
fit.

Every spec is exactly one operation, with exactly one delimiter — there are no
prefixes and no combinations, so raw multi (`{+=}`) and multi truncation do not
exist. Path values are raw by default in display/output and argv contexts
(listing, `--format`/Print, the cwd prompt, and the pager's
`display_directory`), and shell-quoted by default in shell contexts (command
actions, previews, and lessfilter actions). Use `{:}` to force shell quoting,
`{=}` to force raw, or an explicit `S:E` / `S=E` slice to pick the quoting for
that range. `t` and `#` read data rather than a path, so they are always raw:
quote them in the template if you need one argument.

Only recognized placeholders are replaced. Unrecognized ones (`{foo}`,
`{+~2:2}`, `{a:b}`) are left in the output literally, which makes typos visible.
`\{` inserts a literal `{`; the usual C escapes (`\n`, `\t`, `\"`, `\0`, …) are
also decoded.

### The tail column

`{t}` (`{tail}`) reads the tail text of the current item: the `rg` match
context, a database alias, the output of `--transform`, and so on.

`{t}` needs an item, so it stays literal wherever a template is expanded
without one: listing (`--list`, `fs :info`), lessfilter, the cwd prompt, and
command templates while the results cursor is disabled. `{#}` additionally
needs the live result list, so printed output has it literal too.

### Examples

For a current item `/home/you/docs/report.md` with tail `12:3: fn main() {`:

The bare forms below are shown in a raw context (listing/output/cwd/argv). In a
shell context, `{}` and the bare multi specs are quoted like `{:}`.

| Template | Result                                       |
| -------- | -------------------------------------------- |
| `{}`     | `/home/you/docs/report.md`                   |
| `{:}`    | `'/home/you/docs/report.md'`                 |
| `{=}`    | `/home/you/docs/report.md`                   |
| `{0:-1}` | `'/home/you/docs'`                           |
| `{-1:0}` | `'report.md'`                                |
| `{-1=0}` | `report.md`                                  |
| `{1:3}`  | `'home/you'`                                 |
| `{0=0}`  | `/home/you/docs/report.md`                   |
| `{t}`    | `12:3: fn main() {`                          |
| `{#}`    | `7` (the item's index in the results)        |
| `{2~2}`  | `/home…/docs/report.md` (display truncation) |
| `{1~1}`  | `/…/report.md`                               |
| `{1.3}`  | components 1–2 of the cwd, raw               |

With two selected paths `a/first.txt`, `b/second.txt`:

| Template | Result                      |
| -------- | --------------------------- |
| `{+}`    | `a/first.txt b/second.txt`  |
| `{0+1}`  | `a b`                       |
| `{-1+0}` | `first.txt second.txt`      |
| `{+t}`   | tail of every selected item |
| `{+#}`   | `0 1`                       |

## Where templates are expanded

Command and preview templates resolve against the live matchmaker state and can
use the tail column, the results index, and multi-selection. Output templates
carry the printed item, so the tail column works there but there is no results
index. Detached contexts (listing, lessfilter) and the cwd prompt resolve
against a single path.

| Context         | Config / invocation                                                                                                                                            | Notes                                                                                                                                                  |
| --------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Command actions | `[bind]` payloads of `Execute`, `ExecPaged`, `ExecTTY`, `ExecDetached`, `ExecSilent`, `CopyCommand`, `CopyCommandAsync`; `[actions]`/`[rules]` custom commands | shell context: `{}` is the resolved target (the current item, its parent for parent-processing actions, or the cwd while the cursor is locked), quoted |
| Previews        | preview `command`/`layout.command`, `Preview(...)`, `SetPreview`                                                                                               | shell context; environment (`MM_*`, `HIGHLIGHT_*`, …) is supplied by matchmaker                                                                        |
| Output          | `--format` (interactive accept-print and `Print` action)                                                                                                       | raw path values; expanded **once per printed item**: `{t}` is that item's tail, `{+}` sees only that item, `{#}` is literal                            |
| Listing         | `--list`, `fs :info --format`                                                                                                                                  | raw path values; path only: `{t}`/`{#}` stay literal                                                                                                   |
| lessfilter      | action commands in the lessfilter config                                                                                                                       | shell context, so `{}` is quoted; path only                                                                                                            |
| cwd prompt      | `interface.cwd_prompt`                                                                                                                                         | `{}` is the cwd (with `~` collapsed), raw; use `{:}` to quote                                                                                          |
| Pager directory | `pager.toml` `display_directory`                                                                                                                               | argv context, so `{}` is raw; use `{:}` to include quotes                                                                                              |

Status/footer/help lines are matchmaker display templates (`\m`, `\t`,
`{red:text}`), not item templates.

## Escaping and literal braces

- `\{` → literal `{`.
- Shell commands usually contain `{}` only for template placeholders; to pass a
  literal `{}` to a program, escape it: `\{`…`}`.
- Unmatched or nested `{` is emitted literally, as is any spec that does not
  parse.
