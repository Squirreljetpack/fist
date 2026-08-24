# Archive extraction flow

From "user advances into an archive" to "row finalized in the queue UI",
with cleanup at exit. File references are `path:symbol`.

## 0. Pieces

| Layer                                                | Where                                                                |
| ---------------------------------------------------- | -------------------------------------------------------------------- |
| Format engine (detect/list/extract)                  | `fist-copy/src/extract/` (feature-gated per format)                  |
| Scheduler + worker pool + progress/cancel primitives | `fist-copy/src/scheduler.rs`, `copier.rs`, `progress.rs`, `token.rs` |
| Queue rows, pump watcher, toasts/history             | `src/run/queue/mod.rs`, `execute.rs`, `pump.rs`                      |
| Skeleton lifecycle (naming, reuse, freshness)        | `src/unzip/mod.rs`                                                   |

## 0b. Isolation boundary

`fist-copy` is a workspace crate with **zero knowledge of the app**: its
dependencies are `crossbeam-channel`, `log`, `serde`, `thiserror`,
`walkdir`, `libc`, and the archive crates — no ratatui/crossterm, no
`TOAST`, no queue types, no `AbsPath`. The grep-able proof is that every
`crate::` path inside `fist-copy/src` resolves to a fist-copy module.

**Types crossing the boundary** (all plain data, both directions):

| Direction    | Type                                         | Notes                                                                                                       |
| ------------ | -------------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| app → engine | `JobRequest { kind: JobKind, source, dest }` | `JobKind::Copy/Move/Extract`; params are serde `CopyParams`/`MoveParams` (they double as config-file types) |
| engine → app | `TaskSnapshot`                               | plain data via `TaskHandle::snapshot()`; percent, counts, state                                             |
| engine → app | `Vec<String>`                                | via `TaskHandle::log_lines()`; engine-side `log` crate lines are NOT shared — TaskLog is separate           |
| app → engine | `CancelToken` / `TaskHandle::cancel()`       | one-way flag; clones share state                                                                            |

**No callbacks cross the boundary.** The engine never calls up into the
app; communication is strictly submit-then-poll. Everything user-facing
(toasts, queue rows, action history) happens app-side:

- the pump watcher (`pump.rs`) translates snapshots into row atomics,
  toasts, and history
- the unzip registry watcher polls the row status for skeleton lifecycle

Inside the engine the same discipline holds one level down: the per-format
modules receive `extract::ctx::ExtractCtx` — a borrow of the internal
`Progress` + `CancelToken` atomics, not closures or scheduler types — so
format code stays testable without any worker machinery.

Consequence: `fist-copy` could be lifted into its own repository as-is;
only `src/unzip` and `src/run/queue` would move with it conceptually, as
they are pure consumers of the API above.

## 1. Entering an archive — UI thread

`action.rs` sees a file advancing into and calls:

1. `unzip::supported(path)` → `extract::detect(path)`
   - extension map first (`a.tar.gz`, `.zip`, `.7z`, ...), then magic-byte
     sniff of the first bytes; formats whose cargo feature is off never
     match
2. `unzip::init(path)`:
   - canonicalize, look up the registry: re-entry **reuses** the existing
     skeleton while it is *fresh* (skeleton timestamp > archive mtime,
     second granularity); stale or emptied skeletons are rebuilt; a
     previously failed archive reports an error toast and bails
   - `extract::list(source, format)` — cheap listing (zip central dir,
     tar/ar/rar/7z headers; compressed streams decode-and-peek for tar)
   - `alloc_dir`: creates `<tmp>/fist/<pid>/<encoded-path>--<unix-secs>/`
     plus the workdir named after the archive; `skeleton_dir` materializes
     the directory-only tree from the listing (paths validated, files never
     created)
   - registers `Entry { state: Skeleton, row: None, .. }` in the registry
     **before** starting anything, so re-entry can't double-extract
3. `QUEUE::start_extract(source, workdir)` — see §2
4. `must_watch(workdir)` → watcher keeps reloading the workdir unthrottled
   through extraction event storms
5. caller navigates into the workdir immediately; `toast_entering` pushes
   the persistent "Extracting: <name>" toast

## 2. Queue row creation — still synchronous

`queue/mod.rs:QUEUE::start_extract`:

1. submits `JobRequest { kind: Extract(ExtractParams), source, dest }`
   to the global copy scheduler; on rejection → no row at all, entry is
   marked Failed in the registry
2. pushes a row `{ kind: "extract", src: [archive], dst: workdir }`
   already in `Started` state with `task_id` set — extract rows have no
   pending phase and bypass dispatch-time destination resolution
3. `pump::register_task(handle, row)` spawns a pump watcher thread that
   polls the task snapshot every 100 ms

## 3. Execution — copy worker pool

- scheduler's collector sees `is_extract` and enqueues exactly one
  `WorkItem::Extract(ExtractJob)` (no walk phase)
- a pool worker runs `extract/runner.rs:run`:
  1. `detect` again (cheap), log format
  2. dispatch to the per-format module (`tarball` / `zip` / `ar` /
     `stream` / `rar` / `sevenz`)
- each format loop, per entry:
  - check `CancelToken` → cancel aborts between entries by returning
    (dropping decoders kills mid-entry work)
  - `ctx.register_entries(1)` then ok / failed / skipped
  - unsafe paths (absolute, `..`, escaping link targets) are skipped, not
    extracted
- progress semantics: `total_bytes` stays 0, so percent = resolved
  entries / registered entries; the pump writes it into the row's atomic
  `progress`
- failure model: per-entry errors are recorded and the loop continues;
  structural errors (undetected format, unreadable archive) fail the task.
  Rar is the exception — its cursor is consuming, so the first failed
  entry aborts the rest. 7z extracts whole-archive (one unit of work).

## 3b. The pump watcher

One per task, spawned by `pump.rs:register_task` at submission time (both
copy/move dispatch and `start_extract`): a detached thread named
`fist-copy-watch` that owns the `TaskHandle` and translates engine
snapshots into UI state. It never touches the worker pool.

Loop, every 100 ms:

1. `handle.snapshot()` → plain-data `TaskSnapshot`
2. row atomics updated in place (`QueueItemStatus` clones share them with
   the stored row):
   - `progress` = snapshot `percent()` clamped and scaled to `u8` — for
     extractions this is resolved-entries / registered-entries
   - `size` = `total_bytes`, only when non-zero (extractions keep it 0)
3. state dispatch:
   - `Pending` / `Started` → keep polling
   - `CompleteOk` → CAS row `Started → CompleteOk` (rows not in `Started`,
     e.g. failed submissions that were marked directly, are left alone),
     push the `Complete:` toast, append the item to
     `QUEUE_ACTION_HISTORY`, exit
   - `CompleteErr` / `Canceled` → same CAS to `CompleteErr`, push
     `Failed:` toast plus a notice carrying the reason ("canceled" or
     "N file(s) failed"), exit

On exit the handle is dropped from the process-global `COPY_TASKS` map,
which is also what `task_log(task_id)` consults for the queue detail
view's log lines. Because the watcher holds a *clone* of the row's status
atomics, it works identically for rows created by dispatch-time execution
and rows created directly by `start_extract`.

The unzip registry runs a second, much smaller poller alongside this one
(see §4); the two are independent and both tolerate the other finishing
first.

## 4. Completion

Two independent observers fire:

- **pump watcher** (§3b) on terminal snapshot: row state, toasts, action
  history
- **unzip registry watcher** (`unzip/mod.rs:spawn_watch`) on terminal row
  status:
  - `Entry.state` → Complete / Failed (drives reuse-vs-rebuild on next
    entry)
  - pops the persistent "Extracting:" toast

## 5. Cancellation

- queue UI delete-key on a running row → `cancel_or_remove` →
  `scheduler().cancel(task_id)`; workers stop between entries, pump marks
  the row Canceled→CompleteErr path, unzip marks the skeleton Failed
- programmatic: `unzip::cancel(path)` resolves the same row by source path
- scheduler-wide: `Scheduler::shutdown` cancels everything it drains

## 6. Exit

Order in `start.rs`:

1. `queue::shutdown(3s)` — scheduler `cancel_all` + drain workers (stops
   in-flight extractions between entries)
2. `unzip::shutdown()` — spawns the named TASKS blocking task
   `"unzip skeleton cleanup"` = one `remove_dir_all(root())`
3. `TASKS::shutdown(...)` — joins the task, surfacing "Waiting on unzip
   skeleton cleanup" warnings with a hard cap; past the cap the deletion
   finishes detached
4. backstop for exit paths skipping all of this: `libc::atexit` hook also
   removes the root; SIGKILL strands the per-process root for the system
   tmp cleaner

## Format matrix

| Format                          | Crate(s)          | Listing          | Extraction granularity       | Feature                         |
| ------------------------------- | ----------------- | ---------------- | ---------------------------- | ------------------------------- |
| zip                             | `zip`             | central dir      | per-entry                    | `zip`                           |
| tar (+ gz/xz/bz2/zst compounds) | `tar` + decoder   | full stream scan | per-entry                    | `tar`, `gz`, `bz2`, `xz`, `zst` |
| bare gz/bz2/xz/zst              | decoder crates    | header peek      | single unit                  | same                            |
| ar                              | `ar`              | member headers   | per-entry                    | `ar`                            |
| rar                             | `unrar` (C++ SDK) | headers          | per-entry (cursor-consuming) | `rar` (off on aarch64/musl)     |
| 7z                              | `sevenz-rust2`    | header only      | whole-archive                | `sevenz`                        |

Detection is content-first when extensions lie; tar-vs-bare for compressed
streams is decided by peeking the decoded head (magic at offset 257 or a
valid header checksum).
