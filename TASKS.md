improve QUEUE:
Each task gets a log u can open
Copy/Move: Replace copy_with_progress with background worker:
Filter out foo -> foo/bar. No resolve symlinks.
walkdir: flat queue of entries
Preallocate directory
workers: configurable (num_cpus default) pull file tasks from queue, checking for cancellation token
if reflink, do it. (per file)
On finish, apply metadata (configurable)
main scheduler assembles progress reports into total progress displayed in ui
Directory metadata (timestamps and permissions) is applied in reverse hierarchical order only after all nested child files and subdirectories have finished copying. (possible to allow some flag on To field but OOS)

reflink:
consult yazi, use using platform C bindings on macos (libc::copyfile with COPYFILE_CLONE or clonefile(2), dunno windows

Stage 2:
Hard Links & Inode Deduplication
Strategies: Fail, Overwrite, Skip, Rename/Suffix

note: cleanup is not accounted for by TASKS currently
queue dispatch: needs more specific queue kind name
