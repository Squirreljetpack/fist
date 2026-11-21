# fist-docs (site) — documentation guide

Project guide for `fist-docs`: sidebar grouping, terminology, sources of truth, clip index, and site conventions for this site.

Sidebar grouping:

- Getting started: `index`, `getting-started`
- Panes: `panes`, `navigation-in-depth`, `find-pane`, `search-pane`,
  `history-database`, `stash-panes`, `custom-pane`, `app`, `visibility`,
  `sorting` (plus `mount-pane` once complete)
- Menu: `menu-actions`, `queue`, `lua`
- Configuration: `configuration`, `mm.toml`, `lessfilter`
- Mechanics: `shell-integration`, `command-line`, `tools`, `output-templates`

Clip shorthand: each scene exists as `<name>.mp4` with a `<name>.png` poster under `docs/images/`; `splash` and `cwd-prompt` are poster-only.

---

## Terminology

Words the prose uses consistently. Where two spellings exist, the right column wins.

| term         | means                                        | not                               |
| ------------ | -------------------------------------------- | --------------------------------- |
| f:st         | the product, in prose                        | `fist`, `f:ist`                   |
| `fist`       | slugs, identifiers, paths, config dir names  | f:st                              |
| `fs`         | the binary, always in backticks              | fist                              |
| pane         | a view with its own cwd, listing and query   | window                            |
| overlay      | drawn over a pane, takes the keys while open | popup, modal                      |
| prompt       | the query bar; locked or unlocked            | search bar                        |
| query        | what is typed into the prompt                | filter, pattern                   |
| row          | a line in any list                           | item, entry                       |
| tail         | the second column's data                     | metadata, description             |
| preset       | a lessfilter rule table                      | profile                           |
| bind action  | a payload a key runs (`FsAction`)            | command                           |
| context menu | the `alt-e` / `ctrl-e` action list overlay   | action menu, menu overlay         |
| menu action  | a TOML entry under `actions/`                | plugin, script                    |
| kind         | a queue row's executor                       | type                              |
| stash        | a named collection of paths                  | bookmark (except the `alt-b` one) |

Naming rule: `f:st` in prose wherever it is a word; fall back to `fist` only where a slug, path or identifier forces it; never `f:ist`.

---

## Sources of truth

| topic               | files                                                                                      |
| ------------------- | ------------------------------------------------------------------------------------------ |
| `config.toml`       | `src/config/mod.rs`, `panes.rs`, `styles.rs`, `ui.rs`, `src/db/zoxide.rs`                  |
| `mm.toml`           | `src/run/mm_config.rs`, `assets/config/mm.toml`                                            |
| lessfilter          | `src/lessfilter/config.rs`, `assets/config/lessfilter.toml`                                |
| lua                 | `src/lua/`, `assets/plugins/*.md`                                                          |
| menu actions        | `src/menu/`, `src/ui/menu_overlay/`, `assets/config/actions.toml`, `assets/actions/*.toml` |
| queue               | `src/run/queue/`, `src/ui/queue_overlay.rs`, `fist-copy/`, `docs/CONFLICTS.md`             |
| templates           | `docs/TEMPLATING.md`                                                                       |
| panes               | `src/run/pane.rs`, `src/run/populate*.rs`, `src/config/panes.rs`                           |
| navigation          | `src/run/start.rs`, `src/run/reload.rs`, `src/run/binds.rs`                                |
| history             | `src/db/`, `src/db/zoxide.rs`                                                              |
| stash               | `src/run/stash.rs`                                                                         |
| shell               | `assets/shell/shell.zsh`, `src/cli/clap_.rs`                                               |
| command line        | `src/cli/clap_.rs`                                                                         |
| tools               | `src/cli/clap_.rs` (`SubTool`), `assets/help/*.txt`                                        |
| visibility, sorting | `src/config/panes.rs`, `fist-types/src/filters.rs`                                         |

Commands that answer questions directly:

- `fs <sub> --help`, `fs :tool <name> --help`
- `fs --dump-config` piped to a file: the resolved config
- `fs :tool showbinds`: the keymap after every override
- `fs :tool lessfilter --diagnose PATH`: which rule wins, and why
- `fs :tool check`: config and installed menu actions
- `fs :tool action NAME`: run a menu action directly, to test an edit
- `fs :tool types`: types and categories

---

## Clip index

Every scene has a home. Primary reference first.

| clip           | page                | heading                           |
| -------------- | ------------------- | --------------------------------- |
| splash         | index               | *(top intro)*                     |
| nav-pane       | navigation-in-depth | parent-and-advance                |
| find           | find-pane           | working-with-results              |
| search         | search-pane         | query-mode-and-filter-mode        |
| custom         | custom-pane         | a-markdown-notes-manager          |
| history        | history-database    | history-panes                     |
| apps           | app                 | *(top intro)*                     |
| queue          | queue               | the-queue-overlay                 |
| info           | lessfilter          | presets                           |
| image          | lessfilter          | images-and-graphics-protocols     |
| sort           | sorting             | choosing-a-sort                   |
| filters        | visibility          | the-options-overlay               |
| previews       | panes               | previews                          |
| help           | panes               | help                              |
| paged          | lessfilter          | paging-and-headers                |
| action         | mm.toml             | menu                              |
| diff           | menu-actions        | examples                          |
| stash          | stash-panes         | actions                           |
| stash-workflow | stash-panes         | cross-directory-workflows         |
| compress       | menu-actions        | examples                          |
| archive        | navigation-in-depth | browsing-archives-transparently   |
| move           | queue               | executing-and-pasting             |
| queue-edit     | queue               | editing-rows                      |
| queue-complete | queue               | removing-reordering-and-filtering |
| long-copy      | queue               | progress-and-the-status-line      |
| extract        | queue               | extraction-rows                   |
| prompt         | navigation-in-depth | prompt-locking                    |
| cwd-prompt     | output-templates    | the-cwd-prompt-template           |
| shell          | shell-integration   | line-editor-widgets               |
| nav            | shell-integration   | the-nav-function                  |
| undo           | navigation-in-depth | undo-and-redo                     |
| undo-jump      | navigation-in-depth | undo-and-redo                     |
| reel           | getting-started     | tour                              |

Secondary placements:

- `nav-pane` also appears on `getting-started#first-run`.
- `previews` also appears on `lessfilter#presets`.
- `splash` also appears on `sorting#size-sorting`.

## Site conventions

- VitePress configuration lives in `docs/.vitepress/config.mts` (`base: /fist-docs/`, `cleanUrls`, dark appearance, and the Getting started / Panes / Menu / Configuration / Mechanics sidebar groups)
- Media clips and posters live in `docs/images/` using `<video controls preload="none" poster="./images/<name>.png" src="./images/<name>.mp4"></video>`
- The `index` "Browse by topic" grid uses `.zen-sections-grid` and `.zen-section-card` styled in the theme CSS
- Engineering notes in the `fist` repo (`docs/ARCHITECTURE.md`, `docs/CONFLICTS.md`, `docs/TEMPLATING.md`) are internal references, not user-facing site pages
- Every overlay ships a help document under `assets/help/`; direct readers to `alt-h` inside the overlay rather than restating overlay key tables or exposing repo-internal `assets/help/` paths on the site
- `mount-pane` is omitted from the site until the mount pane ships; elsewhere, mention mounting only where navigation depends on it (advance on an archive is a transparent preview; a mount point or `Extract` opens the overlay)
