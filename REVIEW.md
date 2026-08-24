# Code Review: `4f60e4f..HEAD` (unzip v1 extraction engine)

Two commits (`ba0948a` feat: unzipv1, `177562f` checkpoint) refactor archive extraction from a monolithic `src/unzip/mod.rs` with the `decompress`/`sevenz-rust2` crates into a modular, feature-gated engine in `fist-copy/src/extract/`, integrated through the copy scheduler's queue system.

Overall this is a strong, well-structured refactor — clear crate boundary, feature-gated formats, proper cancellation threading. The issues below are ordered by severity.

---

## 🔴 Bugs

### 1. Symlink path traversal in tar extraction

[`tarball.rs:184`](file:///home/archr/gh/_fzs/fist/fist-copy/src/extract/tarball.rs#L177-L187) passes the symlink's **full relative path** (`rel`, e.g. `foo/symlink`) to [`link_target_safe`](file:///home/archr/gh/_fzs/fist/fist-copy/src/extract/safety.rs#L34-L63), which expects the **parent directory** of the link:

```rust
// tarball.rs — current
fn link_target_is_safe<R: Read>(dest: &Path, rel: &Path, entry: &Entry<'_, R>) -> bool {
    let target = entry.link_name().ok().flatten();
    match target {
        Some(t) => link_target_safe(dest, rel, &t),   // ← rel = "foo/symlink"
        None => false,
    }
}
```

`link_target_safe` does `root.join(link_dir)` → `dest/foo/symlink`, so `../../outside` pops `symlink` then `foo`, appearing to stay inside `dest`. But the OS resolves relative to `dest/foo`, so `../../outside` escapes to `dest/../outside`.

**Fix:** pass `rel.parent().unwrap_or(Path::new(""))` instead of `rel`.

### 2. `sweep_others` deletes the just-allocated skeleton

In [`init()`](file:///home/archr/gh/_fzs/fist/src/unzip/mod.rs#L129-L131):

```rust
let workdir = alloc_dir(&source)?;      // skeleton_name() → "...--T1"
extract::skeleton(&workdir, &listing);
sweep_others(&source);                  // skeleton_name() → "...--T2"
```

[`sweep_others`](file:///home/archr/gh/_fzs/fist/src/unzip/mod.rs#L356-L376) calls `skeleton_name(source)` again, generating a new timestamp `T2`. Since `T1 ≠ T2` (even 1ms drift), the keep-name doesn't match the allocated dir — sweep deletes it while extraction is running into it.

**Fix:** pass the allocated skeleton dir name from `alloc_dir` into `sweep_others` instead of regenerating it.

### 3. Extraction `files_ok` double-counted

[`collect_extract`](file:///home/archr/gh/_fzs/fist/fist-copy/src/walker.rs#L158-L171) enqueues a single `WorkItem::Extract` without calling `register_file`. The format extractors call [`ctx.entry_ok()`](file:///home/archr/gh/_fzs/fist/fist-copy/src/extract/ctx.rs#L78-L80) → `prog.file_ok()` for each archive entry. When the work item completes, [`complete_item`](file:///home/archr/gh/_fzs/fist/fist-copy/src/scheduler.rs#L104-L120) calls `prog.file_ok()` **again** for `ItemOutcome::Done`. Result: `files_ok = files_total + 1`.

**Fix:** skip `file_ok()`/`file_failed()` in `complete_item` for extract work items, or have the runner handle its own accounting entirely.

### 4. RAR extraction aborts remaining entries on first error

In [`rar::extract`](file:///home/archr/gh/_fzs/fist/fist-copy/src/extract/rar.rs#L62-L76), a failed `extract_with_base` logs, calls `entry_failed()`, then **breaks** out of the loop. Remaining archive entries are silently lost — no `register_entries`, no `entry_failed`. The function still returns `Ok(())`, masking partial extraction.

> [!NOTE]
> The doc comment on line 7 acknowledges this ("the first extraction error aborts the remaining archive"), but `Ok(())` at the end doesn't signal error to the scheduler. The task may appear successful.

### 5. Permanent lockout on failed extraction

In [`init()`](file:///home/archr/gh/_fzs/fist/src/unzip/mod.rs#L114-L118): if a `.failed` marker exists in the freshest skeleton, the function unconditionally returns `None` with an error toast. There's no staleness check — even if the archive is updated (`mtime > ts`), the failed skeleton blocks re-entry permanently until app restart.

**Fix:** check `ts > mtime` before rejecting; if the archive is newer, fall through to rebuild.

---

## 🟡 Improvements

### 6. `fast_rename` bypasses `ConflictStrategy`

[`fast_rename`](file:///home/archr/gh/_fzs/fist/fist-copy/src/scheduler.rs#L365-L393) calls `fs::rename()` directly. On POSIX, this silently overwrites an existing destination even if the configured strategy is `Fail`, `Skip`, or `RenameSuffix`.

### 7. Walker errors desync `files_total`

In [`walker.rs`](file:///home/archr/gh/_fzs/fist/fist-copy/src/walker.rs#L55-L63), walk/stat errors call `register_file(0)` (incrementing `files_total`) but never call `file_failed()` or `skip_file()`, and don't enqueue a work item. Progress can never reach 100%.

### 8. Bare stream extraction is uninterruptible

[`stream.rs:77`](file:///home/archr/gh/_fzs/fist/fist-copy/src/extract/stream.rs#L69-L83): `io::copy(&mut decoded, &mut out)` copies the entire decompressed stream in one blocking call. No cancellation check or incremental progress during large payloads. Similarly for [`sevenz.rs`](file:///home/archr/gh/_fzs/fist/fist-copy/src/extract/sevenz.rs) which delegates to `decompress_file` with no hook.

### 9. `percent()` ignores `files_skipped` in entry-count mode

[`progress.rs:76`](file:///home/archr/gh/_fzs/fist/fist-copy/src/progress.rs#L72-L92): when `total_bytes == 0`, numerator is `files_ok + files_failed` — skipped entries aren't included. If an archive has many unsafe paths, progress stalls below 100% until terminal state.

### 10. `mtime_secs` returns milliseconds

[`unzip/mod.rs:314`](file:///home/archr/gh/_fzs/fist/src/unzip/mod.rs#L313-L321): the function is named `mtime_secs` but returns `d.as_millis() as u64`. The implementation is correct (matches `skeleton_name`'s millis convention) but the name is misleading.

### 11. Orphaned skeleton on `start_extract` failure

In [`init()`](file:///home/archr/gh/_fzs/fist/src/unzip/mod.rs#L133): if `QUEUE::start_extract` fails and returns `None`, the skeleton directory created by `alloc_dir` is left on disk with no queue row or `.failed` marker — it'll be found as "fresh and populated" next time and erroneously reused (as an empty skeleton).

### 12. Pump watcher thread never exits

[`pump.rs::ensure_watcher`](file:///home/archr/gh/_fzs/fist/src/run/queue/pump.rs) spawns a `loop { sleep(TICK); ... }` thread with no shutdown signal. `shutdown()` drains the scheduler but the watcher thread runs until process exit.

### 13. Unbounded `QUEUE_ACTION_HISTORY` / task storage

- [`execute.rs`](file:///home/archr/gh/_fzs/fist/src/run/queue/execute.rs) pushes completed items to `QUEUE_ACTION_HISTORY` with no size cap.
- [`scheduler.rs`](file:///home/archr/gh/_fzs/fist/fist-copy/src/scheduler.rs): `Inner::jobs` retains all submitted tasks permanently in a `HashMap`.

Both grow without bound over long sessions.

### 14. `ar` extraction breaks on GNU/BSD long filenames

[`ar.rs`](file:///home/archr/gh/_fzs/fist/fist-copy/src/extract/ar.rs): GNU `ar` stores long filenames (>15 chars) in a string table; the archive entries have identifiers like `/0`, `/16`. These start with `/` and are rejected by `is_safe()` as absolute paths.

### 15. 7z triple-parses the archive

[`sevenz.rs`](file:///home/archr/gh/_fzs/fist/fist-copy/src/extract/sevenz.rs) opens and parses the 7z archive three separate times: once to count entries, once in `count_unsafe`, and once in `decompress_file`. A single pass would suffice.

---

## 💡 Minor/Cosmetic

| #  | File                                                                               | Note                                                                                                                                      |
| -- | ---------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| 16 | [`detect.rs`](file:///home/archr/gh/_fzs/fist/fist-copy/src/extract/detect.rs)     | `detect_by_content` reads 8 bytes — misses uncompressed tar magic at offset 257 (`ustar`). Extensionless tars won't be detected.          |
| 17 | [`detect.rs`](file:///home/archr/gh/_fzs/fist/fist-copy/src/extract/detect.rs)     | `has_suffix` rejects exact matches like `.tar.gz` (dotfile edge case where `name.len() == suffix.len()`). Unlikely to matter in practice. |
| 18 | [`safety.rs`](file:///home/archr/gh/_fzs/fist/fist-copy/src/extract/safety.rs)     | `is_safe(Path::new(""))` returns `true` (vacuously true `.all()`). Empty paths should be rejected.                                        |
| 19 | [`skeleton.rs`](file:///home/archr/gh/_fzs/fist/fist-copy/src/extract/skeleton.rs) | `let _ = fs::create_dir_all(...)` silently discards errors. A `log::warn!` would help debugging.                                          |
| 20 | [`zip.rs`](file:///home/archr/gh/_fzs/fist/fist-copy/src/extract/zip.rs)           | Symlinks stored in zip entries are extracted as plain files containing the target path text, not actual symlinks.                         |
| 21 | [`unzip/mod.rs`](file:///home/archr/gh/_fzs/fist/src/unzip/mod.rs#L36)             | Doc comment mentions outdated skeleton root path scheme.                                                                                  |
