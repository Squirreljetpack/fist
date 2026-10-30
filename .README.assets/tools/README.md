# README screenshot & clip harness

Produces the images in `.README.assets/`:

```sh
./.README.assets/tools/setup.sh          # build the demo world (once)
./.README.assets/tools/capture.sh all    # or: capture.sh nav info image
./.README.assets/tools/clip.sh all       # MP4 + poster clips in %%CLIPS/
./.README.assets/tools/clip.sh sort compress archive stash
```

`capture.sh` overwrites the README assets in place. Raw window captures are
kept under `/tmp/fist-demo/shots/`. `clip.sh` records a raw MP4 under
`%%CLIPS/<name>.mp4` plus a framed `<name>.png` poster; the x11grab source
stays under `/tmp/fist-demo/clips/` until it is moved into `%%CLIPS/`.

## How it works

1. `setup.sh` builds a throwaway `fs` world under `/tmp/fist-demo/home`
   (Harborlight, vault, media, seeded history) and writes the CommandSpace
   config.
2. `capture.sh` runs `fs` inside **CommandSpace** (the Alacritty fork) on a
   headless X server, through `bwrap` with that home mounted at `/home/dev`, so
   the screenshots show clean paths like `/home/dev/work/harborlight`.
3. `commandspace-shot` (built from the sibling CommandSpace workspace) owns the
   whole session: it starts Xvfb, launches CommandSpace, moves the window to
   the top-left of the headless screen, types the scene keys with xdotool,
   writes the window to a PNG, and tears the session down. With `--record` it
   also runs ffmpeg/x11grab for the duration of the inputs. It is the only
   thing that manages the process lifetime, so an interrupted capture does not
   leave Xvfb/CommandSpace/ffmpeg processes behind.
4. `frame.py` wraps the capture in a transparent, rounded card with a soft
   shadow. There is no window chrome or title bar; the image is just the
   terminal card and its shadow.

CommandSpace is used instead of VHS/alacritty because it is an Alacritty fork
with Sixel support: text renders with the same font/theme as the user's
alacritty, and `[preview.image] protocol = "sixel"` gives real image previews.
Kitty graphics does not work in WezTerm (it draws U+10EEEE placeholders), and
VHS/xterm.js cannot display either protocol.

## Requirements

- `fs` release build in `target/release/fs`
- CommandSpace release build, default `../commandspace/target/release/commandspace`
  (override with `COMMANDSPACE_DIR`)
- `bwrap`, `Xvfb`, `xdotool`, ImageMagick (`import`), Chromium,
  `sqlite3`, `curl`, `git`, `tar`, `magick`
- `ffmpeg` for `clip.sh` (x11grab capture)
- `7z` for the stash/7z scenario, `ps2pdf` for the paged-preview PDF fixture
- `nano` (the `action` clip and the `custom` chaplet beat open it with
  `Advance`, so they run with `EDITOR=nano`)
- Hack Nerd Font and a Vulkan/GL software rasterizer (llvmpipe is used here)
- `~/.cargo/bin` bound into the sandbox for `fd`, `rg`, `bat`, `eza`, and
  `~/.local/bin` for `7z`

## Notes

- The capture grid is 91 columns x 24 rows at font size 20.0 (Hack Nerd Font),
  giving a 1500x900 terminal card. With 35px transparent padding the framed
  PNG is 1570x970, i.e. 1.618 (the golden ratio). `frame.py` reads the actual
  PNG dimensions, so changing the grid does not require touching it.
- The terminal card is `#282828`; the canvas around it is transparent, and the
  card has no title bar.
- The demo photos are downloaded from Picsum by ID (Unsplash source) into
  `/tmp/fist-demo/cache`. Delete that directory to re-fetch them.
- The `apps` table stores `path` and `cmd` as BLOB (`AbsPath`/`OsStringWrapper`);
  inserting them as TEXT makes the whole Apps pane decode fail and show `0/0`.
  The seed script casts them.
- The `info` README still uses `[panes.nav] preview_layout_index = 1`
  (`config-info.toml`) so mediainfo fits on one line. The combined `previews`
  clip uses the normal right-hand preview.
- SVG is text, so `assets/config/lessfilter.toml` maps `1|ext:svg` to `Image`
  in the `preview` preset and to `Text` in the `extended` preset. Without the
  first rule the normal preview pages raw XML; without the second the Extended
  pager renders the image. Both rules are needed for `previews`.
- The default history prompt is `◷>` for both Files and Folders in
  `assets/config/config.toml`. `setup.sh` overrides the disposable demo config
  with the labeled `◷ files>` / `◷ folders>` prompts.
- The Search pane sets `lock_prompt = "never"`; entering it does not lock the
  query bar. The clip types the query explicitly.
- `capture.sh` scene keys assume the demo world from `setup.sh` (mtimes are
  pinned for the nav/photo ordering).
- `setup.sh` creates `$DEMO/work/fist` as a symlink to the bind-mounted source
  tree (`/home/dev/repo/fist-src`). The Search scene runs from `$DEMO/work`,
  so the repo's own source is searched alongside Harborlight; this is what
  makes the Search screenshot show a larger result set. The link is dangling
  on the host by design; it only resolves inside the capture namespace.
- `clip.sh` now includes 22 scenes:
  - core/doc pages: `search`, `custom`, `history`, `sort`, `filters`, `image`;
  - previews: `previews` (normal preview, directory Info, SQLite `?`, then
    `logo.svg` as an image with its Extended text pager, and the next photo's
    default image preview and Info) and `paged` (PagedPreview on JSON then
    PDF, with pager search);
  - workflows: `action` (open the menu, scroll it, close it, then Advance →
    nano → back), `fileops` (new file,
    new folder, rename, delete), `diff` (diff then compare), `stash`
    (start in the prompt, stash one doc, stash a repo file from the parent,
    then open the stash pane), `compress`
    (cursor-select two files, zip), `archive` (zip + tar.gz, cut a top-level
    file, Parent, paste at the base, enter the tar.gz and its `data/`) and
    `move` (cut, Folders history, paste, Queue).
  - prompt: `prompt` (cwd lock with `Up`, `alt-space`, Find auto-lock);
  - queue: `queue-edit` (delete a row, run into the current directory, delete
    the copy, reopen and re-run the completed row) and `long-copy` (2 GB
    `ctrl-v` copy with the gauge held open);
  - shell: `shell` (`z harb` partial jump, dir / rg / file widgets, edit the
    hit in nano, `git add` two files from the line buffer) and `nav` (`x src`
    into the repo, Find with two `ctrl-d` for folders only, `api` then
    `shift-right` into the package, Options sort by name, second file in nano);
  - pane stack: `undo` (`--sort=name`, cursor-only drill three folders deep,
    undo back out, a sibling path, `Left` twice, undo twice) and `undo-jump`
    (`ctrl-g` history, query a folder, copy a file, undo twice, paste).
- Most queries are typed short: two or three characters that narrow the list
  well enough (`au` for `auth.rs`, `rel` for `release-1.2.0.zip`, `pho` for
  `photos`), since the clips show behaviour rather than exact rows. About a
  third stay full names (`vault`, `auth`, `data`/`cache`/`media`/`photos`,
  `json`/`pdf`) so the reel also shows ordinary typing.
- `custom` seeds more than one screen of Obsidian notes and walks two
  queries: `di` plus cursor movement onto the chaplet note, which `Right`
  opens in nano, then `% Prog` for the beaver series in the Programming
  category. It ends inside the menu's `New` prompt with `Busty Beaver.md`
  typed and never submitted, which is also the poster.
- `compress` selects the first two rows in `docs/` with `Tab`; no file query is
  typed. `sort` selects size first and then changes the same Options sort pane
  to mtime before closing it.
- `filters` walks hidden-only, hidden, Dirs, Files and all with the Options
  modal still open. `search` ends with two `shift-Down` preview scrolls.
- `image` filters to JPGs, moves across several images and scrolls the portrait
  preview. `previews` combines directory Info, the SQLite `?` preview, the SVG
  source pager and the next photo's default image preview and Info. `action`
  opens the menu with `alt-e`, scrolls it, closes it with `Esc`, then presses
  `Right` on the cursor file: with no prompt active `Right` is `Advance`, so no
  `shift-right` is needed.
- `archive` finishes by pressing `Up` on the first row of the mounted `data/`
  pane: `Up` on the top row enters the cwd prompt, which locks even inside an
  extracted archive, so the prompt shows the `/tmp/fist/<id>/unzip/...` root.
  One `Up` too many leaves the prompt again, because `Up` while locked moves.
- `diff` and `action` do not end on their poster frame, so `clip.sh` takes a
  `POSTER_AT` timestamp for them: `diff.png` is the two-file diff rather than
  the compare that closes the clip, and `action.png` is the open menu.
- `history` starts in `work/` and opens `ctrl-g` straight away: the scene types
  no folder name, so the Folders pane it shows is the seeded frecency list.
- `undo` passes `--sort=name`, because nav panes order by mtime and the six
  vault folders are created in the same second, so their tie order is not
  something a cursor-only scene can rely on; sorted by name, three `Down`s land
  on `Programming` and the drill `Right` + `Right` reaches `AI/Multimodal`.
  `undo-jump` is the copy-flavoured one: `ctrl-g`, a query that floats
  `vault/meetings` to the top,
  `ctrl-c` on a standup note, `ctrl-z` twice, `ctrl-v` in `data/`.
- `prompt` holds each lock state for about a second: the redraw is slow enough
  that a shorter hold can be a single frame. `Right` in a prompt is
  `ForwardChar`; use `shift-right` while the prompt is active.
- `stash` runs `--lock-prompt=always` so the pane opens in the query prompt,
  and `alt-space` leaves it; `Esc` does not leave a prompt, it quits the app.
  After `Left` to
  the parent, the cursor sits on the `docs` folder it came from (second-to-last
  row), so two more `Down`s wrap to the top of the list, where `Tab` twice
  selects two repo entries for a three-item stash. The unnamed stash pane takes
  its `⊞>` prompt (with a trailing space) from `[panes.stashes.""]` in the shipped config; `setup.sh`
  only adds the `bookmark` pruning pane, since a second `[panes.stashes.""]`
  table would be a duplicate-key config error.
- `queue-edit` leaves the destination empty and executes from the `inbox`
  pane, so the copy lands in that pane's directory. A relative destination is
  re-resolved against the nav pane on every dispatch, so a typed relative path
  would land somewhere else when the row is re-run from another directory.
  `setup.sh` sets `[queue] clear_completed_on_close = true` so the finished row
  survives closing the overlay; `Enter` commits an in-cell edit, a second
  `Enter` executes, and the queue overlay's `Delete` removes the selected row.
- `long-copy` relies on `setup.sh` appending `[queue.copy] buffer_size = 1536`
  (a top-level `[copy]` table is ignored and silently reverts to defaults) and
  on `work/data/bulk.bin` from `dd`.
- `shell` runs with `FIST_SHELL=1`: `run-fs.sh` execs bash instead of fs, and
  the demo `$HOME/.bashrc` evaluates the generated integration. The rc also
  redefines `__fist_rg_widget` with `>/dev/tty`, because `bind -x` captures
  widget stdout and `lessfilter edit` aborts with "Standard output is not a
  terminal"; `Right` then opens the hit in nano.
- The generated bash dir widget aborts an empty pending line with `kill -INT`
  after `cd`, so readline redraws the prompt for the new directory. bash has no
  `zle push-line && zle accept-line`; a widget without this leaves the old
  prompt (with the old `\w` path) on screen. A non-empty line is left alone.
- The `7z` action accepts absolute paths, so a stash selection spanning
  directories can be archived; 7z stores the basenames.
- The Obsidian clip uses the real `ob-files` launcher from
  `~/.config/matchmaker/presets/obsidian` (override with `OB_PRESET`). It is
  read-only bound into the workspace and opens a demo vault with category
  directories so the tail column stays meaningful.
- `%%CLIPS/` is local-only under the global `/%%*` ignore rule. The MP4s are
  raw 1500x900 CommandSpace recordings; the matching PNG posters are framed
  with `frame.py`. README screenshots come from `capture.sh`, except the stash
  pane and action menu, which use the `stash` and `action` posters.
