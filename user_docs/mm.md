# mm.toml

`mm.toml` configures the matchmaker layer: the results table, previews, the
terminal layout, the overlays f:st adds, and key bindings. The application's
own settings live in [config.toml](config.md).

| File          | Purpose                                                  |
| ------------- | -------------------------------------------------------- |
| `config.toml` | Pane behavior, history, transfers, watcher, path styles. |
| `mm.toml`     | Columns, previews, TUI layout, overlays, key bindings.   |

Because `mm.toml` is mostly matchmaker's configuration format, this page only
describes the sections f:st uses and the overlay options specific to f:st.
For the shared rendering and terminal options — `[ui]`, `[query]`, `[results]`,
`[status]`, `[preview]`, `[preview.image]`, `[header]`, `[footer]`, `[tui]`,
`[overlay]`, `[previewer]`, and `[help]` — consult the
[matchmaker configuration reference](https://squirreljetpack.github.io/matchmaker-docs/configuration).

## Binds

`[binds]` maps triggers to action strings. The available actions, trigger
syntax, and mode-specific bindings are documented in
[Actions](actions.md).

```toml
[binds]
"tab" = ["ToggleSelection", "Down"]
"nav^^alt-n" = "Execute($EDITOR .)"
```

Bindings defined here are merged over the built-in defaults.

## Preview layouts

`[[preview.layout]]` declares the preview layouts that the `preview_layout_index`
settings in [config.toml](config.md) and the preview actions select:

```toml
[[preview.layout]]
side = "right"
percentage = 40
min = 30
max = 120

[[preview.layout]]
side = "top"
percentage = 50
min = 5
max = 50
```

The `[preview]` table and the layouts themselves are matchmaker settings; see
the matchmaker configuration reference for all fields.

## Overlays

### Queue and app views

`[queue]` styles the queue overlay. `[app]` styles the app pane's pending-files
view; the two tables accept the same keys.

| Key                 | Type    | Default                    | Meaning                                    |
| ------------------- | ------- | -------------------------- | ------------------------------------------ |
| `frame`             | frame   | inherits `[overlay].frame` | Border and padding.                        |
| `column_spacing`    | integer | `2`                        | Spaces between columns.                    |
| `cell`              | style   | default                    | Normal cell style.                         |
| `current_style`     | style   | black background           | Style of the current row.                  |
| `selected_style`    | style   | yellow foreground          | Style of selected rows.                    |
| `editing_style`     | style   | black background           | Style of the cell being edited.            |
| `editing_row_style` | style   | default                    | Style of the rest of the row being edited. |

### Options

`[options]` styles the options overlay:

| Key             | Type      | Default         | Meaning                             |
| --------------- | --------- | --------------- | ----------------------------------- |
| `frame`         | frame     | title `Options` | Border and padding.                 |
| `item_fg`       | color     | `"DarkGray"`    | Foreground of option labels.        |
| `item_modifier` | modifier  | none            | Modifiers applied to option labels. |
| `alignment`     | alignment | left            | Alignment of the options columns.   |

`[options.filter]`, `[options.sort]`, and `[options.pane]` style the three
panes inside the overlay. Each accepts `frame` and `alignment`.

### Prompt

`[prompt]` styles the query-bar prompt overlay:

| Key                   | Type    | Default   | Meaning                                               |
| --------------------- | ------- | --------- | ----------------------------------------------------- |
| `frame`               | frame   | all sides | Border and padding.                                   |
| `scroll_padding_left` | integer | `3`       | Characters kept between the cursor and the left edge. |

### Menu

`[menu]` styles the action menu:

| Key                  | Type    | Default                                    | Meaning                                                                         |
| -------------------- | ------- | ------------------------------------------ | ------------------------------------------------------------------------------- |
| `frame`              | frame   | title `Menu`, all sides, padding `2,2,0,1` | Border and padding.                                                             |
| `no_default_aliases` | bool    | `false`                                    | Remove the built-in aliases from builtin items; custom action aliases are kept. |
| `compact`            | bool    | `false`                                    | Remove `Move`, `Copy`, `Open`, and `Trash` from the menu.                       |
| `min_width`          | integer | `16`                                       | Minimum menu width.                                                             |
| `min_height`         | integer | `12`                                       | Minimum menu height.                                                            |
| `[menu.results]`     | table   | `min_width_from_cols = false`              | Column table settings; see the matchmaker `[results]` reference.                |

### Confirmation

`[confirm]` styles confirmation prompts:

| Key                           | Type      | Default                                    | Meaning                                         |
| ----------------------------- | --------- | ------------------------------------------ | ----------------------------------------------- |
| `frame`                       | frame     | all sides, white border, padding `2,2,0,1` | outer border                                    |
| `content_frame`               | frame     | light double-dashed border                 | Frame around any content shown with the prompt. |
| `button_fg` / `button_bg`     | color     | reset                                      | Normal button colors.                           |
| `selected_fg` / `selected_bg` | color     | black on white                             | Selected button colors.                         |
| `hotkey_modifier`             | modifier  | `BOLD`                                     | Modifiers applied to the hotkey letter.         |
| `options_alignment`           | alignment | center                                     | Alignment of the button row.                    |
| `options_gap`                 | integer   | `2`                                        | Spaces between buttons.                         |

### Mounts

`[mount]` styles the mounts picker that appears after a disk image or archive
is mounted:

| Key               | Type  | Default                                      | Meaning                                                          |
| ----------------- | ----- | -------------------------------------------- | ---------------------------------------------------------------- |
| `frame`           | frame | title `Mounts`, all sides, padding `2,2,0,1` | Border and padding.                                              |
| `[mount.results]` | table | `min_width_from_cols = false`                | Column table settings; see the matchmaker `[results]` reference. |

### Locked prompt

`[query_locked]` overrides the query-bar and status-line appearance while the
prompt is locked:

```toml
[query_locked.query]
frame = { sides = "BOTTOM", fg = "red" }

[query_locked.status]
# any partial [status] settings
```

`[query_locked.query]` accepts a partial `[query]` config and
`[query_locked.status]` a partial `[status]` config; the fields are matchmaker
settings.

## Defaults applied by f:st

A few matchmaker settings are set by f:st at runtime:

- The queue, app, options, menu, confirm, and mount frames fall back to
  `[overlay].frame` when unset.
- The preview's initial index is cleared and its initial size is set to 70%.
  When no layout is configured, a single right-side layout running the
  `preview` lessfilter preset is used.
- Unless `--fullscreen` is passed, the terminal layout defaults to `60%` when
  `[tui].layout` is not set.
