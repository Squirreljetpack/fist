# fist-docs (site) — documentation guide

Reference guide for maintaining `fist-docs` (content scope, editorial rules, terminology, page skeletons, and clip placements).

Three goals, in priority order: **up to date** (the binary is the authority), **focused** (no `--help` duplication or UI noise), **reference voice** (see the editorial policy).

Sidebar grouping:

- Getting started: `index`, `getting-started`
- Panes: `panes`, `navigation-in-depth`, `find-pane`, `search-pane`,
  `history-database`, `stash-panes`, `custom-pane`, `app`, `visibility`,
  `sorting` (plus `mount-pane` once complete)
- Menu: `menu-actions`, `queue`, `lua`
- Configuration: `configuration`, `mm.toml`, `lessfilter`, `pager`
- Mechanics: `shell-integration`, `command-line`, `tools`, `output-templates`

Clip shorthand: each scene exists as `<name>.mp4` with a `<name>.png` poster under `docs/images/`; `splash` and `cwd-prompt` are poster-only.

---

## Editorial policy

### The `--help` rule

- If a reader can get it from `fs X --help`, link to `--help` instead of restating it
- Flags, defaults, aliases, and accepted values belong to `--help`, not exhaustive markdown tables (applies across `command-line`, `tools`, and pane pages)
- What earns a section: a concept with a decision or non-obvious interaction (which preset wins, why a sort is soft, when a destination resolves, how `diskspace` builds multi-path trees)
- Config and extension pages (`menu-actions`, `lessfilter`, `configuration`, `mm.toml`) should end their opening section by noting how to validate or test edits (`fs :tool check`, `fs :tool action`, `fs :tool lessfilter --diagnose`), without enumerating what `:tool check` rejects

### No backend mechanics or contributor internals

- `fd` and `ripgrep` are implementation details: describe what a setting does to results, never the CLI flags it passes
- Never expose internal crate/binary names (`fist-copy`, `fist-size`), static/module names (`MEM_STASHES`), internal Rust type/enum names (`CompleteOk`, `PendingErr`, `TransferParams`, `RootStrategy`, `ConflictStrategy`), syscalls/errnos (`O_CREAT | O_EXCL`, `rename(2)`, `EXDEV`), internal task IDs, polling/debounce timings (`100 ms`, percentage blends), or internal helpers (`showerror`)
- Omit contributor-only and debug-build details (`dev.toml`, `*.dev.toml`, `dev.db`, repo paths like `assets/config/pager.toml`); these are user-facing docs
- When a default differs between the binary fallback and `--dump-config`, label it as `(--dump-config)` rather than "shipped config"

### No UI narration or self-evident state

- Describe what an action *does*, not obvious visual mechanics: never narrate that an overlay stays open, that toggling repopulates the pane immediately, that an empty list displays `"Queue is empty"`, that empty columns collapse, that selections clear on push, or exact toast wording
- Skip self-evident UI behaviors: mutually exclusive radio toggles (`dirs` vs `files`), toggle-to-clear on active options, standard readline editing keys, and trivial boundary no-ops (`Parent` at `/`, `Advance` on an empty folder)
- Do not repeat generic pane operations (movement, selections, previews, stashing, file ops) on specialized pane pages (`find-pane`, `custom-pane`); cross-reference `panes` instead

### Reference voice

- These are reference pages, not tutorials: no step lists, no "now you try"
- Write naturally and explain the *intent* of a feature—what problem its design solves and how its pieces work together—in the opening paragraph and the first line of each H2, rather than dryly reciting config fields or inventing UX convenience slogans
- Organization should reflect how a reader understands and uses the feature rather than mirroring struct layouts
- `getting-started` is the one page allowed to sequence things
- Examples are allowed where config is uninstructive without one: the custom pane's note manager, stash setups, `assets/actions` entries
- A clip illustrates a section; it never replaces an explanation

### Page shape

- Title, then one paragraph: what this is and why you would reach for it
- H2s follow the order a reader asks questions, not the order of the code (`getting-started` puts **First run** and **Core keybindings** right after **Install**, before shell setup and config dumping)
- No `Overview` or `Hero` heading; opening content sits directly under the H1
- No `Summary` or `Conclusion`, and no manual `Next:` footer rows (rely on VitePress's built-in `Previous` / `Next` footer)
- Depth stops at H3

### Prose

- Sentence case headings, unnumbered, no "Introduction"/"Advanced"
- Second person, present tense; "you" and "the pane", never "the user"
- One topic per paragraph: break multi-topic blocks (such as terminal requirements + image protocols + build targets, or long flag enumerations) into short bullets or subsections
- A table for enumerations, prose for anything with a decision in it; avoid repeating full config tables from `configuration` on feature pages
- Keep single settings (such as `clear_completed_on_close`) in the operations section they govern rather than giving them their own H2, and drop repetitive table columns when a rule has a single default with one or two exceptions described in prose
- Table cells are noun phrases, one line, no sentences or leading imperative verbs
- Keys in backticks and spelled as the config spells them: `ctrl-f`, `alt-e`, `ctrl-shift-a`
- Code fences carry a language (`shell`, `toml`, `lua`); commands are runnable as written; no `...` elisions; no output unless the output is the point
- At most one admonition per page, and only for something destructive
- Do not say `shipped` i.e. 'shipped `config.toml`', prefer to directly hyperlink `config.toml` (i.e. to the source file on github)

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

When prose and code disagree, the code wins. Verify directly against the source before updating a page.

| topic               | files                                                                                      |
| ------------------- | ------------------------------------------------------------------------------------------ |
| `config.toml`       | `src/config/mod.rs`, `panes.rs`, `styles.rs`, `ui.rs`, `src/db/zoxide.rs`                  |
| `mm.toml`           | `src/run/mm_config.rs`, `assets/config/mm.toml`                                            |
| lessfilter          | `src/lessfilter/config.rs`, `assets/config/lessfilter.toml`                                |
| pager               | `src/config/pager.rs`, `assets/config/pager.toml`                                          |
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
- `fs :tool types`: types and categories

The recordings are the second source of truth: if prose and a clip disagree about what a key does, re-record rather than reword.

---

## Clip index

Every scene has a home. Primary reference first.

| clip           | page                | heading                           |
| -------------- | ------------------- | --------------------------------- |
| splash         | index               | *(top intro)*                     |
| nav-pane       | navigation-in-depth | parent-and-advance                |
| find           | find-pane           | working-with-results              |
| search         | search-pane         | query-mode-and-filter-mode        |
| custom         | custom-pane         | example-a-markdown-notes-manager  |
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
- `paged` also appears on `pager#when-output-gets-paged`.
- `splash` also appears on `sorting#size-sorting`.

## Site conventions

- VitePress configuration lives in `docs/.vitepress/config.mts` (`base: /fist-docs/`, `cleanUrls`, dark appearance, and the Getting started / Panes / Menu / Configuration / Mechanics sidebar groups)
- Media clips and posters live in `docs/images/` using `<video controls preload="none" poster="./images/<name>.png" src="./images/<name>.mp4"></video>`
- The `index` "Browse by topic" grid uses `.zen-sections-grid` and `.zen-section-card` styled in the theme CSS
- Engineering notes in the `fist` repo (`docs/ARCHITECTURE.md`, `docs/CONFLICTS.md`, `docs/TEMPLATING.md`) are internal references, not user-facing site pages
- Every overlay ships a help document under `assets/help/`; direct readers to `alt-h` inside the overlay rather than restating overlay key tables or exposing repo-internal `assets/help/` paths on the site
- `mount-pane` is omitted from the site until the mount pane ships; elsewhere, mention mounting only where navigation depends on it (advance on an archive is a transparent preview; a mount point or `Extract` opens the overlay)
