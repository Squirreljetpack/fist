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
2. `unzip::init(path)` — stateless; every decision is a disk read or a
   queue query:
   - canonicalize, detect, list (`extract::list`: zip central dir,
     tar/ar/rar/7z headers; compressed streams decode-and-peek for tar)
   - **running-row guard**: a started `extract` queue row for this source
     ⇒ cd straight into that row's workdir (covers concurrent re-entry and
     the same-second reallocation edge — no second worker, ever)
   - freshest on-disk skeleton for this source (dir name decodes to the
     archive path): a `.failed` marker inside it reports "Extraction
     failed" and bails; otherwise fresh (`ts > mtime`) *and populated* ⇒
     reuse; stale or emptied ⇒ fall through and rebuild
   - `alloc_dir` creates `<tmp>/fist/<pid>/<encoded-path>--<unix-millis>/`
     plus the workdir named after the archive (app-owned: *where*);
     `extract::skeleton(workdir, listing)` materializes the directory-only
     tree (engine-owned: *what*); an async sweep removes older skeletons
     of the same source, excluding the new one
3. `QUEUE::start_extract(source, workdir)` — see §2
4. `must_watch(workdir)` → watcher keeps reloading the workdir unthrottled
   through extraction event storms
5. caller navigates into the workdir immediately; `toast_entering` pushes
   the persistent "Extracting: <name>" toast

## 2. Queue row creation — still synchronous

`queue/mod.rs:QUEUE::start_extract`:

1. submits `JobRequest { kind: Extract(ExtractParams), source, dest }`
   to the global copy scheduler; on rejection → no row at all
2. pushes a row `{ kind: "extract", src: [archive], dst: workdir }`
   already in `Started` state with the engine task id stamped into its
   status — extract rows have no pending phase and bypass dispatch-time
   destination resolution
3. lazily starts the unified watcher on first submission (nothing can
   forget it)

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
- progress semantics — two tiers, chosen per format; `percent()` prefers
  bytes whenever a byte total was seeded, and falls back to
  resolved-entries / registered-entries when `total_bytes` is 0:
  - **exact bytes** — zip (central-directory sizes, counted `io::copy`)
  - **source bytes** — plain tar, ar, and compressed streams: a counting
    reader wraps the raw file; denominator = file size. Exact for
    uncompressed containers, an honest "archive processed" metric for
    compressed ones. On success `SourceBytes::finish` folds in the
    structural tail (tar zero-block padding, compression trailers) that
    consumers never read, landing at exactly 100%; cancelled/failed tasks
    keep their partial value
  - **entry counts only** — rar and 7z (no interception point)
- failure model: per-entry errors are recorded and the loop continues;
  structural errors (undetected format, unreadable archive) fail the task.
  Rar is the exception — its cursor is consuming, so the first failed
  entry aborts the rest. 7z extracts whole-archive (one unit of work).

## 3b. The unified watcher

ONE detached thread (`fist-copy-watch`), lazily started at the first
engine-backed submission, watching the *scheduler* — not the queue. Its
tick every 100 ms (wrapped in `catch_unwind`: a panicked tick must not
kill all future finalizations):

1. `scheduler().snapshot()` → all `(TaskId, TaskSnapshot)` pairs; the
   engine never evicts jobs, so terminal states are durable and nothing
   can be missed between ticks
2. **discovery**: ids not seen yet are resolved against `QUEUE_STATE` by
   matching `status.task_id()` — exactly once per task, under the lock;
   the resolved status clone (shared atomics with the stored row) is kept
   in the watcher's local cache for the task's lifetime, so removing a
   running row cannot lose its completion
3. **mirror**: progress (u8 percent) and size written into the cached
   status atomics
4. **finalize**: on a terminal snapshot, CAS the row `Started →
   CompleteOk/CompleteErr`. The CAS is the once-only "seen" flag — the
   winner alone pushes the `Complete:`/`Failed:` toast (+ reason),
   appends to `QUEUE_ACTION_HISTORY`, and evicts the cache entry.
   Extract-specific branches: pop the persistent "Extracting:" toast; on
   failure also drop a `.failed` marker into the skeleton dir plus a
   transient "Extraction failed" notice.

Lua/symlink rows self-finalize inline in `execute()` and never appear in
snapshots; the watcher ignores them.

## 4. Completion

One observer: the unified watcher (§3b) — row state, toasts, history,
and the extract-specific `.failed` marker / "Extracting:" toast retire.
There is no unzip-side thread: lifecycle state is *derived* from disk and
queue rows at read time (see §1).

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
