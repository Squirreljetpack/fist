# The Pager

`fs` includes a lightweight built-in pager based on
[minus](https://docs.rs/minus/latest/minus/) for command output, help screens,
and plain-text fallback, and delegates rich file previews to
[`mm pager`](https://squirreljetpack.github.io/matchmaker-docs/pager) when `mm`
is on `PATH`.

## When output gets paged

- **Paged actions**: `ExecPaged` (`ExecutePaged`), `LFPaged`
  (`LessfilterPaged`), and menu actions using `strategy = "ExecPaged"` stream
  their stdout into the built-in minus pager drawn on `/dev/tty` (while keeping
  stdin on the terminal). Quitting the pager kills a still-running command and
  returns to the TUI. `LFPaged(Preview)` (`ctrl-l`) pages the preview through
  the built-in pager (showing single-line metadata for images), while
  `LFPaged(Extended)` (`alt-l`) hands text and image files directly to
  [`mm pager`](https://squirreljetpack.github.io/matchmaker-docs/pager) with
  `PG_INTERACTIVE=true` so images and markdown render natively.
- **Help**: `Help` and overlay help (`alt-h`) open in the built-in minus pager
  on `/dev/tty`.
- **Lessfilter text & extraction**: In `lessfilter`, `Text` steps (`preview`,
  `display`, and non-paged `extended` presets) run `PG_IMAGE=false mm pager -- <path>`
  when `mm` is on `PATH`, falling back to the built-in pager when `mm` is
  absent. `Extract` pipes plain text from `kreuzberg` into the built-in pager.
- **`fs :tool liza`**: Paged `liza` views page interactively through the built-in
  pager when stdout is a terminal.

### Built-in pager controls

| Key     | Action                                      |
| ------- | ------------------------------------------- |
| `q`     | Quit the pager and return to the TUI        |
| `alt-h` | Open minus help                             |
| `y`     | Copy the current selection to the clipboard |
| `Esc`   | Clear the current selection                 |

Standard navigation, search, and toggle keybindings come from
[minus](https://docs.rs/minus/latest/minus/).
