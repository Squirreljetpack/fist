---
name: fist-lessfilter
description: Customizes F:ist's lessfilter.toml - the scored rule table deciding how each file is previewed, displayed, opened, edited, and summarized. Use when changing previews for specific file types, adding custom preview commands or categories, wiring the alternate/edit/open presets, or debugging why a file previews wrongly.
compatibility: Requires the F:ist CLI (`fs`). Rule quality depends on optional tools being on PATH (bat, eza, chafa, kreuzberg, mediainfo, sqlite3, ...); rules can gate on them with `have:`.
---

# F:ist lessfilter configuration

Use this skill to edit `lessfilter.toml`: the file that maps files to **presets** (preview, display, open, edit, ...), where each preset is a list of scored `(actions, patterns)` rules. The highest-scoring rule wins and its actions run on the target file.

## Source of truth

```sh
fs --dump-config            # writes the default lessfilter.toml next to config.toml on first run
fs :tool lessfilter --diagnose display PATH   # print detected data + winning rule + commands, run nothing
```

Read the shipped `assets/config/lessfilter.toml` before inventing syntax; it is commented and covers most shapes.

If the request is ambiguous, clarify which preset is meant ("preview" usually means the `?` pane, but "display in terminal" means the `display` preset) and whether the change should apply everywhere (`default`) or to one surface.

## Presets and where they are used

| Preset                    | Used by                                                            |
| ------------------------- | ------------------------------------------------------------------ |
| `default`                 | Merged (appended) into every other preset at dispatch time         |
| `preview`                 | The preview pane (`?`, the default right-hand preview)             |
| `display`                 | Terminal display; the `l` shell alias                              |
| `extended`                | Verbose/interactive terminal display; `la` alias; `alt-l` maximize |
| `info`                    | Metadata/raw info; `ll` alias; the informative preview (`alt-/`)   |
| `open`                    | System open (`fs :open` defers here)                               |
| `edit`                    | The `n` alias and the in-app Advance action on files               |
| `alternate`, `alternate2` | Free slots for user binds (`alt-8` pages `alternate` by default)   |

Invoke any preset directly while testing:

```sh
fs :tool lessfilter <preset> [--header=true|false] [--no-exec] [--arg X]... PATH...
```

Preset name aliases: `p` review, `d` display, `x` extended, `i` info, `o` open, `e` edit.

The setting `tracked_presets` (default: edit, alternate, extended) lists presets whose invocations record the visited file into the history database — add a preset there if running it should count as "opening" the file.

## File layout

```toml
# top-level settings
infer = "FileFormat"        # Guess | Infer | FileFormat — how mime/type detection runs

[rules]                     # one key per preset; each is a list of [actions, patterns] pairs
default = [...]
preview = [...]

[actions]                   # custom actions referenced by name from rules
sqlite = 'sqlite3 {} ...'

[categories]                # named mime groups usable as cat: rules and `-t` groups
ebook = ["application/epub+zip", "application/x-mobipocket-ebook"]
```

Category keys must not collide with built-in category names (image, video, audio, document, source, ...). Categories defined here also become custom groups for the find pane's `-t` flag.

## Rule entries

Each entry is `[actions, patterns]`; both are lists. For a given file every pattern is scored, the best total wins, ties go to the first entry, and all of the winning entry's actions run in order.

```toml
preview = [
    [["Application"], ["application"]],
    [["sqlite"], ["application/vnd.sqlite3", "have:sqlite3"]],
]
```

An entry may carry an execution policy instead of a bare action list:

```toml
[[{ kind = ["Extract", "Metadata"], execution = "Until" }, ["cat:compressed"]]]
```

- `All` (default): run every action regardless of failures.
- `Abort`: stop at the first failing action.
- `Until`: stop after the first succeeding action.

### Score modifiers

| Syntax    | Meaning                             |
| --------- | ----------------------------------- |
| `>{n}     | {rule}`,`{n}                        |
| `+{n}     | {rule}`/`+{rule}`                   |
| `-{n}     | {rule}`/`-{rule}`                   |
| `<{n}     | {rule}`                             |
| `^        | {rule}`/`^{rule}`                   |
| `!{rule}` | invert the test                     |
| `\{rule}` | treat modifier characters literally |

Default scores: `application` Max(60), `glob` Max(50), `child` Max(50), `ext` Max(30), `mime` Max(20), `cat` Max(20), `*` Max(0); `have`, `filetype`, `git` are Req.

Because `default` rules are appended **after** the preset's own rules, a plain `ext:` match in `default` (Max 30) beats a plain `mime:` override in a preset (Max 20). Use explicit prefixes (`40|mime:image/*`, `>50|...`) when combining.

### Patterns

| Pattern              | Tests                                                                                    |
| -------------------- | ---------------------------------------------------------------------------------------- |
| `glob:{pat}`         | Full path against a glob                                                                 |
| `child:{pat}`        | Any child name matches (siblings, for a file)                                            |
| `ext:{e}`, `.{e}`    | File extension                                                                           |
| `mime:{m}`, `{m}`    | MIME type (`text/plain`, `image/*`, `*/*`)                                               |
| `cat:{name}`         | Built-in or configured category                                                          |
| `have:{cmd}`         | Executable exists on PATH (a requirement, not a score)                                   |
| `filetype:{t}`       | file, directory, symlink, blockdevice, chardevice, executable, empty, socket, pipe, text |
| `git`                | Path inside a git work tree                                                              |
| `application`, `app` | Platform application bundle/launcher/executable                                          |
| `*`                  | Any path                                                                                 |

## Built-in actions

- `Directory` — directory listing per preset (eza-backed via `fs :tool liza`).
- `Text` — in-process pager (bat → minus); `edit` infers `$VISUAL`.
- `Image` — chafa render (terminal graphics protocol auto-detected).
- `Application` — icon render with directory fallback.
- `Metadata` — metadata dump (mediainfo/kreuzberg class tools), stats for directories.
- `Extract` — extract readable content from documents/archives into the pager (requires `kreuzberg`); commonly paired as `["Extract", "Metadata"]`.
- `Open` — system open.
- `Header` — the app header line.
- `None` — renders nothing.

In the `open` and `alternate` presets these standard actions collapse to a system open of the path.

## Custom actions

Names not matching a builtin resolve to `[actions]`. The value is a command template executed under the user's shell; `{...}` placeholders expand against the target path:

| Template | Expands to                                                                     |
| -------- | ------------------------------------------------------------------------------ |
| `{}`     | Absolute target path, shell-quoted                                             |
| `{a:b}`  | Path components slice, quoted (negative indices from end; `{:-1}` = file name) |
| `{=...}` | Same slices, unquoted/raw                                                      |
| `{.}`    | Slices of the current working directory instead                                |

```toml
[rules.alternate]
# (entries)
[actions]
code = 'code --add {}'
rainfrog = 'rainfrog --url "sqlite://{.}?mode=ro"'
```

Keep templates single-purpose; multi-step pipelines belong in a script referenced by absolute path.

## A practical workflow

1. Reproduce first: `fs :tool lessfilter --diagnose <preset> <path>` prints mime/kind/filetype/perms, the winning rule with its score breakdown, and the exact commands. Most bugs are visible here without executing anything.
2. Add the narrowest rule that fixes the case, in the right preset (or `default` if it should apply everywhere).
3. Score deliberately: prefer `have:` requirements for tool-dependent actions so systems without the tool fall through cleanly.
4. Verify with `--no-exec` (resolve and report, don't run), then run the preset directly, then check inside the TUI (`?` for preview, `alt-/` info, `alt-l` extended).
5. Remember paged output honors `PG_RAW`, `PG_FLAGS`, `PG_LANG`, `PG_FILENAME`, and `HIGHLIGHT_LINE` environment variables when testing rendering.

## Common failure modes

- **Rule never fires:** its best possible score loses to a default rule (plain `mime:` = 20 vs plain `ext:` = 30). Raise it explicitly (`50|mime:...`).
- **Rule fires in one preset but not another:** it was added to `preview` while the complaint was about `display`/`info`; remember `default` applies to all.
- **Custom action "not defined":** the rule names an action missing from `[actions]` (names are matched exactly).
- **Preview blank but no error:** the winning command produced nothing on stdout, or a required tool silently failed — check with `--diagnose`, then run the printed command by hand.
- **Everything opens in the editor:** an overly broad `edit` rule (e.g. `*/*`) shadowed narrower ones.
- **TOML escaping confusion:** literal strings (`'...'`) pass backslashes through unchanged; write shell escapes once, do not double them.
