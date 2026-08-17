# F:ist

F:ist is a fast and intuitive search tool for the filesystem.

// video

> *Skeptic: What would you say is your biggest strength?*
>
>> Speed, simplicity, power, capability, extensibility, customizability, adaptability, minimalism, flexibility, utility, efficiency, versatility, robustness, reliability, precision, clarity, elegance, performance, responsiveness, portability, composability, modularity, interoperability, scalability, maintainability, resilience, configurability, programmability, expressiveness, concision, practicality, pragmatism, openness, autonomy, control, freedom, agency, coherence, consistency, discoverability, usability, intuitiveness, observability.
>>
>> <p align="right">— <b>f:ist</b></p>

# Installation

Install from GitHub releases[^20]:

```shell
curl -fsSL https://raw.githubusercontent.com/Squirreljetpack/fist/main/install.sh | sh
```

Install the required dependencies:

```shell
# dependencies (optional)
cargo install bat fd-find eza ripgrep
```

Optionally, setup shell integration:

```
# Only zsh support for now
echo "\neval $(fs :tool shell)" >> ~/.zshrc # or whatever the startup file of your respective shell is.
```

Call as:

- `fs`: Directory navigation
- `fs [..paths] pattern`: interactive find
- `generate_paths | fs :custom`: enriched fuzzy searching of paths
- `z [query]`: directory jump (requires [shell integration](#shell-integration))

[^20]: f:ist is also on cargo: `cargo install fist`, but this method is not recommended as fist is currently missing features

# Commands

### Bindings overview

- `Up`/`Down`: Navigate (or `Up` in the initial position to to enter prompt).
- `Left`/`Right`: Back/Enter.
- `Enter`: Default (system) open.
  - `Alt-Enter`: Print / Alternate open.
  - `Ctrl-Enter`: Open in background.
  - `alt-n`: Open folder in editor.

---

- `ctrl-f`/`ctrl-r`: Find files / Search text.
- `ctrl-g`: History view (Folders and files).
- `ctrl-z`/`ctrl-y`: Undo / Redo.

---

- `ctrl-x`/`ctrl-c`/`ctrl-v`: Move, Copy, Paste.
- `delete/shift-delete`: Trash/Delete.
- `ctrl-e`: Open menu.
- `ctrl-u`: Open queue.
- `ctrl-p` : Open filters.
- `ctrl-s`/`alt-h`: Toggle hidden.
- `ctrl-d`: Toggle contextual visibility.

---

- `Tab`: Toggle select.
- `alt-enter`: Print.
- `?`: toggle preview.
- `alt-/`: toggle [informative](#lessfilter) preview.
- `ctrl-l`: Maximize preview.
- `alt-l`: Maximize [extended](#lessfilter) preview.
- `/` and `~`: Jump to home
- `ctrl-[1-9]`: Autojump to item
- `ctrl-0`: Autojump to prompt

For a full list of binds, press `alt-h` within the app. [^1]

[^1]: For more information on bindings (how they are defined, key testing, and default generic binds), see [matchmaker](https://github.com/Squirreljetpack/matchmaker).

# Panes

### Nav

To begin, call `fs` without any positional arguments.

<img src=".README.assets/nav-pane.png" alt="Navigation pane" style="height:400px;" />

Once inside, you can navigate and re-enter from other panes by pressing the left/right arrow keys (corresponding to the `Parent`/ `Advance` actions).

### Find

You can search through all files recursively by

- using the subcommand: `fs :: [OPTIONS] [PATHS]... [PATTERN]`
- by calling `fs` directly with the same arguments
- or by triggering the `Find` action (`ctrl-f`) in-app.

<img src=".README.assets/filters-overlay.png" alt="Filters overlay" style="height:309px;" />

The results will be available for filtering, navigating, editing, previewing etc. Filtering and sort order can be adjusted through the [Filters overlay](#filters).

> [!NOTE]
>
> f:ist uses fd for this internally, and that search parameters can be passed through directly following `--`. However, it is not a strict wrapper and several differences in behavior in the command specification exist:
>
> - The last positional argument is treated as the query instead of the first
> - queries beginning with `.` auto-enables the inclusion of hidden files
> - Default parameters, directory-specific ignores, and other parameters can be set in the [config](./src/config/mod.rs#L257).
> - The `-t` (type) flag has be overloaded to support more conditions. In addition to file types (`directory/d, symlink/l, ..etc.` ), it now supports extensions (`-t .ext`), pre-set categories (`image/i, video/v`), and custom categories as well.

### Search

You can perform a full text search

- using the subcommand: `fs : [OPTIONS] [PATTERNS]... [-- <RG_ARGS>...]`
- or by triggering the `Search` action (`ctrl-r`) in-app.

In f:ist, each result supports two columns: the main filepath column, and a secondary context column[^3].

In this pane, the context column contains the query matches (and any requested context lines around them).

This pane operates in a query and a filter mode, which can be switched between[^4]:

- In *query mode*, the results are (dynamically) populated with all text matches of a given query (your input).
- In *filter mode*, the results are filtered to only lines matching your input.
- By default, the filter applies to the main (first) column. To switch to filtering the second column, type `%` (i.e. `path_filter % context_filter`)
- The current query/filter of the inactive mode is displayed above your input.
- In query mode, multiple queries (of which any should match) are seperated by whitespace. Queries containing whitespace can be grouped together by single quotes. Single quotes can be escaped as `\'`.
- The default mode treats the given queries as *regexes* (as opposed to the filter input, which does not). This can be toggled, or the default [reconfigured](#configuration).

> [!NOTE]
>
> When the active item is `advance`/`executed` on, the matched line and column are saved in the environment variables `HIGHLIGHT_LINE` and `HIGHLIGHT_COLUMN`. If your system has a compatible editor, the `Lessfilter::Edit` action can automatically open the file to the corresponding position -- otherwise, you can configure this manually.

<img src=".README.assets/search-pane.png" alt="Search pane" style="width: 700px;" />

[^3]: In the previous panes, the secondary column was simply empty and therefore not displayed.

[^4]: via the same action.

### Stream/Custom

f:ist can also accept **arbitrary lists of files from a command** or **input stream** through the `:custom` subcommand (`fs :custom [CMD]...`, where an empty command reads from stdin), where all the usual operations are available:

- directory traversal
- file create/edit/delete/custom actions relative to the current item/directory.
- enriched display
- full text search
- reversible actions
- preview
- filtering and sorting
- and so on.

The following is an example script for browsing directories of markdown notes:

```zsh
### --- ob-notes -- ###

#!/bin/zsh

# This first command demonstrates the use of fs as a wrapper for fd,
# by use of the `--list` and `--` parameters:
# `--list` (available for all panes), starts fs non-interactively,
# while arguments after `--` passed through to `fd`.
# The effect however, is simply to list all folders in a given folder.
fs -t d --list $OBSIDIAN_HOME . -- --max-depth 1 |
while read -r line; do
  # This command finds all markdown files, and prints them in a custom format:
  # {a:b} is a slicing syntax for path components
  # {-1:} means take all components including and after the last
  # omitting either end takes the full range in that direction
  # 3 different delimiters are supported for slicing: `:`, `=` and `.`
  # `:` targets the current item and adds single quotes around it
  # `=` targets the current item without single quotes
  # `.` targets the current working directory without single quotes
  # `{}` prints the full path.
  #
  # Here, the effect is to print alongside each note their containing "vault".
  fs -t .md --list --format '{=}\t{-1.}' $line .
done |
# This command browses the results. Note that a bare `fs` call no longer
# consumes piped input -- browsing a listing is now an explicit `fs :custom`:
# --opener: use this program to open the selected file
# --tail-sep: use this delimiter to split each input line into a Path
#                   and a Context (the tail column)
# --transform: a lua function (path, tail) -> (path, display, tail). The input path
#                 is absolute, the input tail is the --tail-sep remainder; a missing
#                 path omits the entry; missing display/tail keep the current values.
#                 Note: Files can be supplied with the `@` prefix.
fs :custom --opener ob-open \
  --tail-sep $'\t' \
  --transform '
local path, tail = ...
local display = (tail and tail ~= "") and ("/" .. path):match("^.-/" .. tail:gsub("%W", "%%%1") .. "/(.*)") or path
return path, display:gsub("^/+", ""):gsub("%.md$", ""), tail
'
```

```shell
### --- ob-open -- ###

# This script takes a filepath, and opens it with Obsidian.
# We pass the uri to fs :o so that it records it in our history, which we can later access using `fs :file`.

uri() {
  print -nl $@ | sed 's/ /%20/g; s/\//%2F/g'
  # or more reliably, print -nl $@ | jq -sRr @uri
}
fs :o "obsidian://open?path=$(uri $1)"
```

<img src=".README.assets/custom-stream-directory-preview.png" alt="Custom stream directory preview" style="height:177px;" /> <img src=".README.assets/custom-stream-new-note.png" alt="Creating a new note" style="height:177px" />

### History

f:ist records the **files, directories and applications** that you've visited in a local database, where they are displayed in the `Files`/`Folders` (`ctrl-g`) and `Apps` panes, sorted by relevance[^6].

<img src=".README.assets/history-pane.png" alt="History pane" style="width:500px" />

The *Files* and *Folders* panes are most useful when integrated into the ambient context where you usually access files. For example, the [shell](#shell-integration), or a [command launcher](#dependencies).

### Named stashes

`PushStash(name)` adds the selection (or the current directory while the cursor is disabled) to a named stash, and `OpenStash(name)` switches to its pane. Stash panes are transient or database-backed collections of paths, useful for scratch space or bookmarking.

### App

The apps pane comes prepopulated from the existing applications on your system, and can be accessed either through

- `fs :o -w [..FILES]` on the command-line
- the `open with` [menu action](#menu)
- a custom binding for the `App` action.

The `App` action has no default key binding. `fs :o --list` prints the
currently known application paths without opening the app pane.

<img src=".README.assets/app-pane.png" alt="Apps pane" style="width:360px;" />

It can be used to select a launch method for a given set of files (provided through the command line, or collected in the app view's pending files).

[^6]: frequency, recency, and similarity to query.

### Additional notes

Panes can be navigated between using the `Undo/Redo` actions.

Every pane has a **Options overlay** (`ctrl-p`), with settings for filtering, sorting, and other pane-specific controls for the displayed results.

<img src=".README.assets/filters-overlay.png" alt="Options overlay" style="height:360px;" />

For more information on any of the panes, run `fs [pane] --help` with the appropriate subcommand (i.e. `:rg`).

# Actions

f:ist supports a flexible "system" custom file-actions:

### Menu

The **Menu** (`ctrl-e`) houses all the actions available in the current context.

Custom actions can be added in the `[menu]` section of the config. They consist of 3 parts:

- **Action**: The script to execute -- see [here](#templating) for how placeholders are resolved.
- **Conditions**: The various conditions which must be satisfied to show this action in the menu.
- **Execution**: Parameters controlling how the action is executed.

### Queue

<img src=".README.assets/queue-overlay.png" alt="Queue overlay" style="height:400px;" />

The **Queue** overlay (`ctrl-u`) lists the pending file operations. Rows show their kind, source, destination, and progress, and can be edited, rearranged, removed and executed from the overlay. `Undo`/`Redo` cycles between filters to narrows the overlay to a single queue kind.

`Move` and `Copy` enqueue items under the `move` and `copy` kinds. The `Paste` (`ctrl-v`) keybind is available to executes every queued `copy`, `move` and `symlink` item without enterring the overlay, transferring files into the active directory.[^7] `ExecuteQueue(selector)`, `Enqueue(kind)` and `ClearQueue(selector)` are also available for binding.

Menu actions with the `Queue`/`QueueBatch` strategies enqueue their targets under the action's key; on execution the action's lua script runs once per queued item with `(paths, dst, nav_cwd)`.

[^7]: Although safeguards exist to keep these alive and prevent data loss during normal application execution and shutdown, if reliability is crucial it might be safer to define your own custom actions to perform, manage and monitor these actions externally. Ideas and contributions in this area are welcome!

# Tools

f:ist comes with several secondary subcommands for reference and utilitary purposes. They can listed with `fs :tool`.

### Shell integration

Only zsh is supported for now.

The output of `fs :tool shell` will, when sourced, provide the jump and jump+open functions:

The jump function (`z`) is a replacement for `cd`, except that incomplete queries are matched to a most likely destination drawn from the unified f:ist database. This behavior is inspired by zoxide[^13].

> [!NOTE]
>
> In addition, a couple special queries can be used to start an interactive search:
>
> - the only argument is a valid path: `cd`.
> - no arguments: interactively select from history.
> - last argument is `.` : interactively search subdirectories of the best match.
> - otherwise: cd into the best match for the search term (if one exists).[^9]
>
> One final change from zoxide is the introduction of the `history.refind` setting in the [config](#configuration).
> When no match is found, or when the top result is the current directory, this setting causes the the interactive interface to be started.

[^9]: There is one final case: if the last argument is `./`: z interactively navigates the best match. If you have [aliases](#aliases) enabled, this is also just `Z`.

The jump+open function (`zz`) is an analogous replacement for [`lessfilter edit`](#lessfilter): if the query head exists, it opens the target(s) in the editor. Otherwise the query is passed to `z`, and the editor opens in the destination.

##### Additional

Including the `--aliases` flag will add a few simple alias definitions into the initialization:

- [lessfilter](#lessfilter)
- `lz`: directory display
- `l`: lessfilter (display preset)
- `la`: lessfilter (extended preset)
- `ll`: lessfilter (info preset)
- `n`: edit (lessfilter with edit preset)
- `o`: [open](#app)
- `Z`: jump (`z`), then navigate
  - In case your shell doesn't support uppercase function names, this one can be renamed like so: `fs :tool shell --aliases --shell csh --nav-name x`.
- `zf`: recent files history

For speed and safety, it is recommended pass your actual shell through to `--shell`.[^10]

[^10]: Another optimization you can make is to cache the generated command: my [zcomet fork](#https://github.com/Squirreljetpack/zcomet) supports this.

### Lessfilter

The previewer is controlled by the lessfilter tool.

The lessfilter tool dispatches to 9 presets:

- `preview`: the preview pane
- `display`: terminal display
- `extended`: terminal interaction or verbose display
- `info`: metadata and raw information
- `open`: system open
- `edit`: editing
- `alternate` and `alternate2`: extra presets for custom use

<img src=".README.assets/lessfilter-info-preview.png" alt="The info preset, using mediainfo to display metadata on a folder of images" style="width: 600px;" />

Each preset is configured by a rules table in the [config file](#https://github.com/Squirreljetpack/fist/blob/main/assets/config/lessfilter.toml). Each rule is a pair (Actions, Patterns), and for a given file, the rule whose patterns score the highest is selected -- its actions are invoked on the target file.

The patterns can be prefixed with a score modifier which dictates how the score is modified by a successful match of the pattern - if this is omitted, the default score modifier for the pattern is used.

The score modifiers are:

- Add/Sub (n): Add/Sub (n) to the current score.
- Max/Min (n): Take the max/min of the current score with (n) for the new score.
- Req: Set the score to 0 if the test fails.

The patterns and their default scores are:

- `glob`: match the full path — `Max(50)`
- `child`: match a child name (or a sibling name for a file) — `Max(50)`
- `ext`: match a file extension — `Max(30)`
- `mime`: match a MIME type such as `text/plain` or `image/*` — `Max(20)`
- `cat`: match a built-in or configured file category — `Max(20)`
- `application`: match a platform-specific application bundle, launcher, or executable — `Max(60)`
- `*`: match any path — `Max(0)`
- `have`: require an executable to exist — `Req`
- `filetype`: require a matching filesystem type — `Req`
- `git`: require a path inside a Git work tree — `Req`

`Req` makes the whole rule score zero when its test fails. `Max(0)` is useful
as a universal test alongside a stronger condition, but does not select a rule
by itself.

Though the syntax has many parts, configuration should be fairly straightforward. F:ist comes with a sane set of defaults with wide coverage for a variety of filetypes, and declaring overrides is as simple as declaring the desired action together with the conditions which it requires. For example:

```toml
### --- lessfilter.toml -- ###

preview = [
    # ...
    # On an file with mime-type sqlite-3 and a system with sqlite3, this rule gets a score of 20.
    [["sqlite"], ["application/vnd.sqlite3", "have:sqlite3"]],
    # ...
]

# When invoking the edit action (in `fist` or through the `n` alias),
# any file belonging to this category will be opened with the system's default preferred application.
# Since this rule has minimal priority (at most 1), any subsequent rule will override it.
edit = [
    [
        ["Open"],
        [
            "1|cat:document",
            "1|cat:spreadsheet",
            "1|cat:email",
            "1|cat:academic",
        ],
    ],
]
```

The built-in actions are:

- `Directory`
- `Application`
- `Text`
- `Image`
- `Extract`
- `Metadata`
- `Open`
- `Header`
- `None`

`Extract` is selected inside a rule alongside other actions; it is not one of
the presets listed above.

Additional actions can be defined with shell syntax. For example:

```toml
[rules]
alternate = [
    [["code"], ["*/*"]],
]
[actions]
code = 'code --add {}'
```

Note that certain default previews will not display without the required [dependencies](#dependencies).

### Types

A list of all supported types, used by the `-t` parameter of the [find subcommand](#find) and the `cat` condition of the [lessfilter](#lessfilter).

### Others

fs exposes several of the more generally useful functionalities it uses internally:

- `diskspace`: A stupid fast parallel directory tree/size lister, using the same background worker that is used in the main app for sorting by size.

- `trash`: cross platform tool for easily moving files to the system trash.

- as for the rest, run `fs :tool` to see a full listing.

# Configuration

Configuration is presently only documented in the source files: [Main Config](./src/config/mod.rs), [Panes](./src/config/panes.rs), [Styling](./src/config/styles.rs), [Miscellaneous UI](./src/config/ui.rs), [Lessfilter](./src/lessfilter/config.rs), [Lessfilter](./src/lessfilter/config.rs), [Matchmaker Config](./src/run/mm_config.rs)[^12].

# Additional

### Dependencies

- fd-find
- ripgrep
- bat (preview)
- eza (optional: preview)
- chafa (optional: preview)
- kreuzberg (optional: preview)
- mediainfo (optional: preview)

Conversely, fist integrates into [CommandSpace](https://github.com/Squirreljetpack/command-space), which you may also enjoy checking out.

### Notes

- The `New` action creates a directory if the target ends with a path seperator[^11].
- The command that spawns programs can be delegated to a process manager. For example, using [pueue](https://github.com/Nukesor/pueue):

```toml
# config.toml

[misc]
spawn_with = ["pueue", "add", "-g", "apps", "--"]
```

[^11]: `/` on unix and `\` on windows

### Examples

List every git repository (by locating its `.git` directory) under a couple of directories, sorted by last modification, mapping each `.git` folder to its parent:

```shell
fs -a --transform '
local path, tail = ...
local parent = path:gsub("/%.git$", "")
if parent ~= path then
    return parent, parent:match("[^/]+$")
end
return path, path
' --sort mtime $HOME/gh $SSdir '\\.git$'
```

---

[^12]: For more information on this one, you can refer to the matchmaker documentation [here](https://github.com/Squirreljetpack/matchmaker/blob/main/matchmaker-lib/src/config.rs).

[^13]: f:ist uses an improved implementation by default, although the simple [zoxide](https://github.com/ajeetdsouza/zoxide) implementation can be enabled by disabling lambda:

    **Event clock**: zoxide score decay is coupled to wall-clock time. The problem is that after any extended period of inactivity, all scores decay toward zero, and the first directories visited on return immediately dominate the ranking regardless of prior history ([see](https://github.com/jghub/ze/tree/master)). f:ist replaces wall-clock time with an event clock: each cd action advances the clock by one tick. The clock stands still during inactive periods and no score decay occurs during such periods.

    **Scoring**: Items are sorted by score. In zoxide, the score is computed as count * recency bucket. f:ist replaces this with a monoexponential decay kernel. The decay rate is controlled by `lambda` (default `8e-3`, equating to a half-life of about 87 actions).

    **Unified database**: f:ist maintains a single SQLite database tracking files, directories, and applications together.

    **Pruning**: Pruning happens automatically and lazily once db exceeds a certain size. For more information, see `fs :tool bump --help`.

    **Interactive fallback**: When no match is found, or when the top result is the current directory, f:ist can be configured to start an interactive search interface instead of failing.
