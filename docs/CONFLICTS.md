# Conflict strategies across fist

Where name collisions can happen, which policy governs each, and what
actually happens. Three separate layers exist on purpose:

| Layer                         | Where                                                            | Governs                                                                                               |
| ----------------------------- | ---------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| **Atomic reservation**        | `cba::claim` (`reserve_file` / `reserve_dir`)                    | The mechanism. Wins names via `O_EXCL`/`create_dir`; no policy, only outcomes (`Reserved` / `Taken`). |
| **Per-entry conflict**        | `fist_copy::ConflictStrategy` (`QueueConfig.copy/move.conflict`) | File-level collisions while a transfer writes entries.                                                |
| **Root destination strategy** | `fist_copy::RootStrategy` (`QueueConfig.copy/move.root`)         | What a transfer does when the target root itself exists.                                              |

---

## 1. Transfer engine (`fist-copy`)

### 1.1 Root-level conflicts — `RootStrategy` (files *and* directories)

Resolved **at submit time**, synchronously on the dispatching thread:
rejections surface immediately as `SubmitError::Claim` (real
reason, no async "0 file(s) failed" notices); Rename claims hold real
handles that the worker writes through. Applies once per task when the
resolved destination already exists.

| Strategy             | File target                                      | Directory target                                |
| -------------------- | ------------------------------------------------ | ----------------------------------------------- |
| `Rename` *(default)* | duplicate as `file_1.ext` (claimed)              | duplicate as `dir_1/` (claimed)                 |
| `Overwrite`          | replace contents; identity (`dst == src`) errors | ⚠️ contents cleared before transfer (inode kept) |
| `Merge`              | skipped (file-merge unimplemented)               | kept, entries merged                            |
| `Fail`               | `SubmitError::Claim` at submit                   | `SubmitError::Claim` at submit                  |

Special cases: move-onto-itself = successful no-op; copy-onto-itself
follows the table (default duplicates); nested destinations (`dst` inside
`src`) always error.

### 1.2 `ConflictStrategy` — entries inside a copied/merged tree

Inner-entry collisions only; the root is decided by §1.1 and marked
*fresh*, bypassing this layer entirely.

| Strategy       | Existing file          | Task result   | Notes                                                                                                                                                  |
| -------------- | ---------------------- | ------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `Overwrite`    | truncated and replaced | `CompleteOk`  | Never claims; if the target is a **directory**, fails honestly with *"cannot overwrite: destination is a directory"* instead of a downstream `EISDIR`. |
| `Fail`         | untouched              | `CompleteErr` | Error names the blocking path.                                                                                                                         |
| `Skip`         | untouched              | `CompleteOk`  | Counted as `files_skipped` only (not `files_ok`).                                                                                                      |
| `RenameSuffix` | untouched              | `CompleteOk`  | Writes `name_1.ext`, `name_2.ext`, … claimed atomically via `cba::claim`; up to 9999 alternatives, then error.                                         |
| `Abort`        | untouched              | `Canceled`    | First collision cancels the whole task.                                                                                                                |

Quirk: `RenameSuffix` against a target that is an existing *directory*
produces a sibling **file** named `existingdir_1`. Reachable only by
explicitly addressing the directory without a separator (see §3).

### 1.3 `RootStrategy` — directory transfer, target directory exists

Checked once per task at submit time (see §1.1); this section restates
the directory column per strategy.

| Strategy             | Existing target                        | Data lands in                                       | Task result                                                                                                                                                                          |
| -------------------- | -------------------------------------- | --------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `Merge`              | kept                                   | inside it                                           | Per-entry `ConflictStrategy` decides collisions.                                                                                                                                     |
| `Rename` *(default)* | kept                                   | `target_1/` (first free suffix, claimed atomically) | Claim holds a real reserved directory from submit onward.                                                                                                                            |
| `Overwrite`          | ⚠️ **contents cleared before transfer** | fresh copy of it                                    | Destructive by design: the inode is kept so the name never vacates, but an aborted transfer leaves the target emptied. A recoverable soft-overwrite is a possible future refinement. |
| `Fail`               | kept                                   | nowhere                                             | `SubmitError::Claim` at submit.                                                                                                                                                      |

Target absent: all strategies behave identically (fresh copy).

### 1.4 Same-filesystem move fast path

Moves with deletion attempt atomic replacement through the claim first — one syscall,
no copying. When `try_replace` succeeds, the move completes immediately. If replacement
is unavailable (e.g. cross-device `EXDEV`), it falls back to the full streaming copy
and clean-up.

---

## 2. Queue dispatch layer

| Situation                                                         | Behavior                                                                                                                                                                                                                      |
| ----------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Transfer row (`copy`/`move`/`symlink`), `data` empty, no nav pane | Filtered before dispatch; stays `Pending` (never counts as started).                                                                                                                                                          |
| Transfer row, explicit `data` (absolute or relative-to-nav)       | Executed without nav. `desired_path` appends the source's file name only when `data` is empty or ends in a separator — an existing-directory `data` without separator is treated as a literal file target and fails per §1.1. |
| Script rows                                                       | Run regardless of nav (they receive it as an argument).                                                                                                                                                                       |

---

## 3. Interactive actions (menu overlay)

| Action                      | Policy                                                                                                                                                                             | Existing target                                                                                                                |
| --------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| **New** (`TODO`/name)       | Exact-only `reserve_file`; trailing `/` reserves a directory instead                                                                                                               | Taken ⇒ skipped toast, nothing touched (previously this silently truncated an existing file / no-op'd on dirs).                |
| **NewDir** (`mkdir`)        | Exact-only `reserve_dir`                                                                                                                                                           | Taken ⇒ skipped toast. Success toast shows the *claimed* path; optional jump follows it.                                       |
| **Rename**                  | Exact-only `reserve_file_all` / `reserve_dir_all`: an exclusive leaf claim after inventing any missing destination parents; the rename itself runs through `Reserved::try_replace` | Existing target ⇒ "Already exists" error, old path untouched; a failed rename rolls back the placeholder and invented parents. |
| **Paste / queue transfers** | §1 engine strategies via `QueueConfig`                                                                                                                                             | Dispatch-time path computation is inference only (§2); collisions decided at write time.                                       |

---

## 4. Archive extraction

Extraction has no conflict strategy by design: every entry goes into a
**fresh skeleton workdir** allocated per archive (reused only while newer
than the archive). Collisions with unrelated files cannot occur; unsafe
entry paths are skipped, never merged into existing trees. Re-entering a
failed archive surfaces a `.failed` marker instead of retrying or merging.

---

## 5. Guarantees summary

- Every `Claim` from `cba::claim` was won by an exclusive syscall — no
  check-then-create windows anywhere reservations are used.
- Engine-side claiming happens at *write time*, on worker threads, so
  concurrent transfers can never interleave two entries onto one name.
- Interactive actions that don't reserve (**New**) are documented as such;
  migrating them is mechanical once desired.

---

## 6. Helper inventory

The small functions everything above is built from:

| Helper                                 | Location                          | Role                                                                                                                                                                                                                                          |
| -------------------------------------- | --------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `reserve_file` / `reserve_dir`         | `cba::claim`                      | The atomic primitive: win a name via `O_EXCL`/`create_dir`, optionally walking suffixed alternatives (`Naming`). Returns a held handle / path, or `Taken`.                                                                                    |
| `reserve_file_all` / `reserve_dir_all` | `cba::claim`                      | The same claim with missing ancestor directories invented first (`create_dir_all` on the parent). Invented levels ride on the claim; `FileClaim::rollback` / `DirClaim::rollback` remove the untouched leaf and repay them while still empty. |
| `Naming`                               | `cba::claim`                      | Fallback format for reservations (`prefix`, `suffix`; default `_` / ``). Most call sites pass `None` — exact name or skip.                                                                                                                    |
| `auto_dest`                            | `utils/path.rs`                   | Interactive-input splitter: absolute-ifies a prompt value against the pane's cwd; `Err(dir)` when the input ends in a separator (directory intent). Feeds New/NewDir.                                                                         |
| `desired_path`                         | `utils/path.rs`                   | Transfer inference: empty `dst` ⇒ into nav; directory-ish `dst` ⇒ append source file name; otherwise `dst` as-is. No policy, no existence checks — collisions are claimed later at write time.                                                |
| `transfer_dest`                        | `queue/execute.rs` (`pub(crate)`) | Combines `data` + nav per the dispatch rules of §2, then delegates to `desired_path`.                                                                                                                                                         |

# COPIER

                  ┌─────────────────────────────────┐
                  │        Scheduler::submit        │
                  └────────────────┬────────────────┘
                                   │
              ┌────────────────────┴───────────────────┐
              ▼                                        ▼
    JobKind::Transfer                        JobKind::Extract
    ┌───────────────────────────────┐        ┌───────────────────────────────┐
    │       submit_transfer         │        │        submit_extract         │
    │  - resolve_root (fast-paths)  │        │  - TaskEntry + TaskLog        │
    │  - JobCtx + rollback_levels   │        │  - JobCtx (ExtractParams)     │
    │  - spawn_collector            │        │  - enqueue WorkItem::Extract  │
    └──────────────┬────────────────┘        └──────────────┬────────────────┘
                   │                                        │
                   ▼                                        │
    ┌───────────────────────────────┐                       │
    │   collector_run & collect     │                       │
    │  - walkdir traversal          │                       │
    │  - DirMetaTracker tree stack  │                       │
    │  - enqueue WorkItem::File/Link│                       │
    └──────────────┬────────────────┘                       │
                   │                                        │
                   └───────────────────┬────────────────────┘
                                       │
                                       ▼ (crossbeam channel)
                        ┌───────────────────────────────┐
                        │          worker_loop          │
                        │  - rx.recv_timeout()          │
                        │  - copier::execute()          │
                        │  - job.complete_item()        │
                        └──────────────┬────────────────┘
                                       │
                                       ▼
                        ┌───────────────────────────────┐
                        │          try_finish           │
                        │  - check walk_done & tracker  │
                        │  - CAS TaskState (Ok/Err/Canc)│
                        │  - repay_invented on failure  │
                        └───────────────────────────────┘
