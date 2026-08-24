# Conflict strategies across fist

Where name collisions can happen, which policy governs each, and what
actually happens. Three separate layers exist on purpose:

| Layer | Where | Governs |
|---|---|---|
| **Atomic reservation** | `cba::claim` (`reserve_file` / `reserve_dir`) | The mechanism. Wins names via `O_EXCL`/`create_dir`; no policy, only outcomes (`Reserved` / `Taken`). |
| **Per-entry conflict** | `fist_copy::ConflictStrategy` (`QueueConfig.copy/move.conflict`) | File-level collisions while a transfer writes entries. |
| **Directory-target merge** | `fist_copy::MergeStrategy` (`QueueConfig.copy/move.merge`) | What a directory transfer does when the target directory itself exists. |

---

## 1. Transfer engine (`fist-copy`)

### 1.1 `ConflictStrategy` — file entry already exists at destination

| Strategy | Existing file | Task result | Notes |
|---|---|---|---|
| `Overwrite` | truncated and replaced | `CompleteOk` | Never claims; if the target is a **directory**, fails honestly with *"cannot overwrite: destination is a directory"* instead of a downstream `EISDIR`. |
| `Fail` | untouched | `CompleteErr` | Error names the blocking path. |
| `Skip` | untouched | `CompleteOk` | Counted as `files_skipped` only (not `files_ok`). |
| `RenameSuffix` | untouched | `CompleteOk` | Writes `name_1.ext`, `name_2.ext`, … claimed atomically via `cba::claim`; up to 9999 alternatives, then error. |
| `Abort` | untouched | `Canceled` | First collision cancels the whole task. |

Quirk: `RenameSuffix` against a target that is an existing *directory*
produces a sibling **file** named `existingdir_1`. Reachable only by
explicitly addressing the directory without a separator (see §3).

### 1.2 `MergeStrategy` — directory transfer, target directory exists

Checked once per task, before the walk.

| Strategy | Existing target | Data lands in | Task result |
|---|---|---|---|
| `Merge` | kept | inside it | Per-entry `ConflictStrategy` decides collisions. |
| `Rename` *(default)* | kept | `<target>_1` (first free suffix, claimed atomically) | Logged: "transferring into … instead". |
| `Overwrite` | ⚠️ **removed before transfer** | fresh copy of it | Destructive by design: aborted transfers leave nothing. A recoverable soft-overwrite (rename aside, delete on success) is a possible future refinement. |
| `Fail` | kept | nowhere | `CompleteErr`: "target directory already exists". |

Target absent: all strategies behave identically (fresh copy).

### 1.3 Same-filesystem move fast path

Moves with deletion attempt `fs::rename(src, dst)` first — one syscall,
no copying. POSIX rename silently replaces, so the fast path runs only
when replacement is allowed at every level the source touches:
destination free, or `ConflictStrategy::Overwrite` (+ `MergeStrategy::
Overwrite` when moving a *directory*). Otherwise it falls back to the
full walk, where the tables above apply.

---

## 2. Queue dispatch layer

| Situation | Behavior |
|---|---|
| Transfer row (`copy`/`move`/`symlink`), `data` empty, no nav pane | Filtered before dispatch; stays `Pending` (never counts as started). |
| Transfer row, explicit `data` (absolute or relative-to-nav) | Executed without nav. `desired_path` appends the source's file name only when `data` is empty or ends in a separator — an existing-directory `data` without separator is treated as a literal file target and fails per §1.1. |
| Script rows | Run regardless of nav (they receive it as an argument). |

---

## 3. Interactive actions (menu overlay)

| Action | Policy | Existing target |
|---|---|---|
| **New** (`TODO`/name) | Exact-only `reserve_file`; trailing `/` reserves a directory instead | Taken ⇒ skipped toast, nothing touched (previously this silently truncated an existing file / no-op'd on dirs). |
| **NewDir** (`mkdir`) | Exact-only `reserve_dir` | Taken ⇒ skipped toast. Success toast shows the *claimed* path; optional jump follows it. |
| **Rename** | Exact-only `reserve_file` (no fallback — you typed the name you want) | Taken ⇒ "Already exists" error, old path untouched. Reservation failure cleans the empty placeholder. |
| **Paste / queue transfers** | §1 engine strategies via `QueueConfig` | Dispatch-time path computation is inference only (§2); collisions decided at write time. |

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

| Helper | Location | Role |
|---|---|---|
| `reserve_file` / `reserve_dir` | `cba::claim` | The atomic primitive: win a name via `O_EXCL`/`create_dir`, optionally walking suffixed alternatives (`Naming`). Returns a held handle / path, or `Taken`. |
| `Naming` | `cba::claim` | Fallback format for reservations (`prefix`, `suffix`; default `_` / ``). Most call sites pass `None` — exact name or skip. |
| `auto_dest` | `utils/path.rs` | Interactive-input splitter: absolute-ifies a prompt value against the pane's cwd; `Err(dir)` when the input ends in a separator (directory intent). Feeds New/NewDir. |
| `desired_path` | `utils/path.rs` | Transfer inference: empty `dst` ⇒ into nav; directory-ish `dst` ⇒ append source file name; otherwise `dst` as-is. No policy, no existence checks — collisions are claimed later at write time. |
| `transfer_dest` | `queue/execute.rs` (private) | Combines `data` + nav per the dispatch rules of §2, then delegates to `desired_path`. |
