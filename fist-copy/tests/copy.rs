use std::fs;
use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant};

use fist_copy::{
    CancelToken, ConflictStrategy, CopyParams, JobKind, JobRequest, MoveParams, ReflinkMode,
    Scheduler, SchedulerOptions, TaskState,
};

fn tmp(name: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(name)
        .tempdir()
        .expect("tempdir")
}

fn write_file(
    p: &Path,
    data: &[u8],
) {
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).expect("mkdir");
    }
    let mut f = fs::File::create(p).expect("create");
    f.write_all(data).expect("write");
}

fn sched(workers: usize) -> Scheduler {
    Scheduler::new(SchedulerOptions {
        workers: std::num::NonZeroUsize::new(workers).unwrap(),
    })
}

fn copy_req(
    src: impl Into<std::path::PathBuf>,
    dst: impl Into<std::path::PathBuf>,
) -> JobRequest {
    JobRequest {
        kind: JobKind::Copy(CopyParams::default()),
        source: src.into(),
        dest: dst.into(),
    }
}

fn move_req(
    src: impl Into<std::path::PathBuf>,
    dst: impl Into<std::path::PathBuf>,
) -> JobRequest {
    JobRequest {
        kind: JobKind::Move(MoveParams {
            copy: CopyParams::default(),
            delete_source: true,
        }),
        source: src.into(),
        dest: dst.into(),
    }
}

fn await_terminal(
    h: &fist_copy::TaskHandle,
    timeout: Duration,
) -> TaskState {
    let start = Instant::now();
    loop {
        let s = h.snapshot().state;
        if s.is_terminal() {
            return s;
        }
        assert!(
            start.elapsed() < timeout,
            "timed out waiting for task to finish"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn tree_copy_preserves_content_structure_and_metadata() {
    let src = tmp("fc-src");
    let dst = tmp("fc-dst");
    let s = src.path();

    write_file(&s.join("a.txt"), b"alpha");
    write_file(&s.join("nested/deep/b.bin"), &[1u8; 4096]);
    fs::create_dir_all(s.join("empty/inner")).expect("dirs");

    let f = s.join("a.txt");
    fs::set_permissions(&f, std::os::unix::fs::PermissionsExt::from_mode(0o750))
        .expect("chmod file");
    let old = fs::FileTimes::new().set_modified(std::time::SystemTime::UNIX_EPOCH);
    set_times_path(&f, old);

    let out = dst.path().join("out");
    let sch = sched(2);
    let h = sch.submit(copy_req(s, &out)).expect("submit");
    assert_eq!(
        await_terminal(&h, Duration::from_secs(30)),
        TaskState::CompleteOk
    );

    assert_eq!(fs::read(out.join("a.txt")).expect("read"), b"alpha");
    assert_eq!(
        fs::read(out.join("nested/deep/b.bin")).expect("read"),
        &[1u8; 4096]
    );
    assert!(
        out.join("empty/inner").is_dir(),
        "nested empty dirs must be created"
    );

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let copied_mode = fs::metadata(out.join("a.txt"))
            .expect("meta")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(copied_mode, 0o750);
    }

    let want_mtime = fs::symlink_metadata(&f)
        .expect("src meta")
        .modified()
        .expect("mtime");
    let got_mtime = fs::symlink_metadata(out.join("a.txt"))
        .expect("dst meta")
        .modified()
        .expect("mtime");
    assert_eq!(secs(want_mtime), secs(got_mtime), "file mtime preserved");

    let src_root_mtime = fs::symlink_metadata(s)
        .expect("root src")
        .modified()
        .expect("mtime");
    let dst_root_mtime = fs::symlink_metadata(&out)
        .expect("root dst")
        .modified()
        .expect("mtime");
    assert_eq!(
        secs(src_root_mtime),
        secs(dst_root_mtime),
        "directory mtime applied only after children finished (reverse-hierarchical)"
    );

    let snap = h.snapshot();
    assert_eq!(snap.percent(), 100.0);
    assert_eq!(snap.copied_bytes, snap.total_bytes);
    assert_eq!(snap.files_failed, 0);
}

fn set_times_path(
    p: &Path,
    t: std::fs::FileTimes,
) {
    let f = fs::OpenOptions::new()
        .write(true)
        .open(p)
        .expect("open for times");
    f.set_times(t).expect("set times");
}

fn secs(t: std::time::SystemTime) -> u64 {
    t.duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[test]
fn symlinks_are_recreated_not_followed() {
    let src = tmp("fc-sym-src");
    let dst = tmp("fc-sym-dst");
    let s = src.path();

    write_file(&s.join("real.txt"), b"target-content");
    #[cfg(unix)]
    std::os::unix::fs::symlink("real.txt", s.join("link.txt")).expect("symlink");
    #[cfg(unix)]
    std::os::unix::fs::symlink("/definitely/not/here", s.join("dangling"))
        .expect("dangling symlink");

    let out = dst.path().join("out");
    let sch = sched(2);
    let h = sch.submit(copy_req(s, &out)).expect("submit");
    assert_eq!(
        await_terminal(&h, Duration::from_secs(30)),
        TaskState::CompleteOk
    );

    #[cfg(unix)]
    {
        let l = fs::symlink_metadata(out.join("link.txt")).expect("link exists");
        assert!(l.file_type().is_symlink(), "must stay a symlink");
        assert_eq!(
            fs::read_link(out.join("link.txt")).expect("readlink"),
            Path::new("real.txt")
        );
        assert_eq!(
            fs::read_link(out.join("dangling")).expect("readlink dangling"),
            Path::new("/definitely/not/here")
        );
    }
}

#[test]
fn dir_into_itself_is_rejected() {
    let src = tmp("fc-self");
    write_file(&src.path().join("x"), b"x");
    let sch = sched(1);
    let err = sch
        .submit(copy_req(src.path(), src.path().join("sub")))
        .expect_err("must reject");
    assert!(matches!(err, fist_copy::SubmitError::IntoItself { .. }));
    let err2 = sch
        .submit(copy_req(src.path(), src.path()))
        .expect_err("identity must reject");
    assert!(matches!(err2, fist_copy::SubmitError::IntoItself { .. }));
}

#[test]
fn move_same_filesystem_uses_rename_fast_path() {
    let src = tmp("fc-mv-src");
    let dst = tmp("fc-mv-dst");
    write_file(&src.path().join("big.bin"), vec![7u8; 100_000].as_slice());

    let out = dst.path().join("moved.bin");
    let sch = sched(2);
    let h = sch
        .submit(move_req(src.path().join("big.bin"), &out))
        .expect("submit");
    assert_eq!(
        await_terminal(&h, Duration::from_secs(10)),
        TaskState::CompleteOk
    );
    assert!(!src.path().join("big.bin").exists(), "source must be gone");
    assert_eq!(
        fs::read(&out).expect("moved content"),
        vec![7u8; 100_000].as_slice()
    );
}

#[test]
fn move_with_delete_disabled_behaves_like_copy() {
    let src = tmp("fc-mvk-src");
    let dst = tmp("fc-mvk-dst");
    write_file(&src.path().join("f"), b"keep-me");

    let out = dst.path().join("f");
    let sch = sched(1);
    let req = JobRequest {
        kind: JobKind::Move(MoveParams {
            copy: CopyParams::default(),
            delete_source: false,
        }),
        source: src.path().to_path_buf(),
        dest: out.clone(),
    };
    let h = sch.submit(req).expect("submit");
    assert_eq!(
        await_terminal(&h, Duration::from_secs(10)),
        TaskState::CompleteOk
    );
    assert!(src.path().join("f").exists());
    assert!(out.exists());
}

#[test]
fn overwrite_replaces_existing_destination_content() {
    let src = tmp("fc-ovr-src");
    let dst = tmp("fc-ovr-dst");
    write_file(&src.path().join("f"), b"NEW");
    write_file(&dst.path().join("f"), b"OLD-CONTENT-LONGER");

    let sch = sched(1);
    let h = sch
        .submit(copy_req(src.path().join("f"), dst.path().join("f")))
        .expect("submit");
    assert_eq!(
        await_terminal(&h, Duration::from_secs(10)),
        TaskState::CompleteOk
    );
    assert_eq!(fs::read(dst.path().join("f")).expect("read"), b"NEW");
}

#[test]
fn cancel_mid_copy_stops_and_marks_canceled() {
    let src = tmp("fc-cancel-src");
    let dst = tmp("fc-cancel-dst");
    write_file(
        &src.path().join("large.bin"),
        vec![3u8; 32 * 1024 * 1024].as_slice(),
    );

    let params = CopyParams {
        buffer_size: std::num::NonZeroUsize::new(64 * 1024).unwrap(),
        workers: std::num::NonZeroUsize::new(1).unwrap(),
        ..Default::default()
    };

    let out = dst.path().join("large.bin");
    let sch = sched(1);
    let h = sch
        .submit(JobRequest {
            kind: JobKind::Copy(params),
            source: src.path().into(),
            dest: out.clone(),
        })
        .expect("submit");

    let start = Instant::now();
    while h.snapshot().copied_bytes == 0 {
        assert!(start.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(1));
    }
    h.cancel();
    assert_eq!(
        await_terminal(&h, Duration::from_secs(10)),
        TaskState::Canceled
    );

    let snap = h.snapshot();
    assert!(snap.files_ok + snap.files_failed <= 1);
}

#[test]
fn missing_source_is_reported() {
    let src = tmp("fc-miss-src");
    let dst = tmp("fc-miss-dst");
    let sch = sched(1);
    let err = sch
        .submit(copy_req(src.path().join("nope"), dst.path().join("nope")))
        .expect_err("missing source");
    assert!(matches!(err, fist_copy::SubmitError::SourceMissing(_)));
}

#[test]
fn reflink_auto_falls_back_gracefully_and_produces_correct_content() {
    let src = tmp("fc-refl-src");
    let dst = tmp("fc-refl-dst");
    write_file(&src.path().join("data.bin"), vec![9u8; 200_000].as_slice());

    let params = CopyParams {
        reflink: ReflinkMode::Auto,
        conflict: ConflictStrategy::Overwrite,
        ..Default::default()
    };

    let out = dst.path().join("data.bin");
    let sch = sched(2);
    let h = sch
        .submit(JobRequest {
            kind: JobKind::Copy(params),
            source: src.path().join("data.bin"),
            dest: out.clone(),
        })
        .expect("submit");
    assert_eq!(
        await_terminal(&h, Duration::from_secs(30)),
        TaskState::CompleteOk
    );
    assert_eq!(fs::read(&out).expect("read"), vec![9u8; 200_000].as_slice());
}

#[test]
fn single_symlink_job_copies_the_link_itself() {
    use std::os::unix::fs::symlink;

    let src = tmp("fc-sl-src");
    let dst = tmp("fc-sl-dst");
    symlink("nowhere", src.path().join("lnk")).expect("symlink");

    let sch = sched(1);
    let h = sch
        .submit(copy_req(src.path().join("lnk"), dst.path().join("lnk")))
        .expect("submit");
    assert_eq!(
        await_terminal(&h, Duration::from_secs(10)),
        TaskState::CompleteOk
    );
    assert_eq!(
        fs::read_link(dst.path().join("lnk")).expect("readlink"),
        Path::new("nowhere")
    );
}

#[allow(dead_code)]
fn unused(_: CancelToken) {}

fn params_with(conflict: ConflictStrategy) -> CopyParams {
    CopyParams {
        conflict,
        ..Default::default()
    }
}

#[test]
fn conflict_fail_keeps_existing_dest_and_fails_task() {
    let src = tmp("fc-fail-src");
    let dst = tmp("fc-fail-dst");
    write_file(&src.path().join("f"), b"NEW");
    write_file(&dst.path().join("f"), b"OLD");

    let out = dst.path().join("f");
    let sch = sched(1);
    let h = sch
        .submit(JobRequest {
            kind: JobKind::Copy(params_with(ConflictStrategy::Fail)),
            source: src.path().join("f"),
            dest: out.clone(),
        })
        .expect("submit");
    assert_eq!(
        await_terminal(&h, Duration::from_secs(10)),
        TaskState::CompleteErr
    );
    assert_eq!(
        fs::read(&out).expect("read"),
        b"OLD",
        "existing dest must be untouched"
    );
    assert_eq!(h.snapshot().files_failed, 1);
}

#[test]
fn conflict_skip_leaves_dest_and_counts_skipped() {
    let src = tmp("fc-skip-src");
    let dst = tmp("fc-skip-dst");
    write_file(&src.path().join("f"), b"NEW");
    write_file(&dst.path().join("f"), b"OLD");

    let out = dst.path().join("f");
    let sch = sched(1);
    let h = sch
        .submit(JobRequest {
            kind: JobKind::Copy(params_with(ConflictStrategy::Skip)),
            source: src.path().join("f"),
            dest: out.clone(),
        })
        .expect("submit");
    assert_eq!(
        await_terminal(&h, Duration::from_secs(10)),
        TaskState::CompleteOk
    );
    assert_eq!(
        fs::read(&out).expect("read"),
        b"OLD",
        "existing dest must be untouched"
    );
    let snap = h.snapshot();
    assert_eq!(snap.files_skipped, 1);
    assert_eq!(snap.files_failed, 0);
}

#[test]
fn conflict_rename_suffix_writes_free_sibling() {
    let src = tmp("fc-ren-src");
    let dst = tmp("fc-ren-dst");
    write_file(&src.path().join("report.txt"), b"NEW");
    write_file(&dst.path().join("report.txt"), b"OLD");

    let out = dst.path().join("report.txt");
    let sch = sched(1);
    let h = sch
        .submit(JobRequest {
            kind: JobKind::Copy(params_with(ConflictStrategy::RenameSuffix)),
            source: src.path().join("report.txt"),
            dest: out.clone(),
        })
        .expect("submit");
    assert_eq!(
        await_terminal(&h, Duration::from_secs(10)),
        TaskState::CompleteOk
    );

    assert_eq!(
        fs::read(&out).expect("read"),
        b"OLD",
        "existing dest must be untouched"
    );
    let sibling = dst.path().join("report_1.txt");
    assert_eq!(fs::read(&sibling).expect("read sibling"), b"NEW");
}

#[test]
fn conflict_abort_cancels_the_task_on_first_conflict() {
    let src = tmp("fc-abort-src");
    let dst = tmp("fc-abort-dst");
    write_file(&src.path().join("a"), b"NEW");
    write_file(&src.path().join("b"), b"NEW");
    write_file(&dst.path().join("a"), b"OLD");
    write_file(&dst.path().join("b"), b"OLD");

    let sch = sched(1);
    let h = sch
        .submit(JobRequest {
            kind: JobKind::Copy(params_with(ConflictStrategy::Abort)),
            source: src.path().to_path_buf(),
            dest: dst.path().to_path_buf(),
        })
        .expect("submit");
    assert_eq!(
        await_terminal(&h, Duration::from_secs(10)),
        TaskState::Canceled,
        "a conflict under Abort must cancel the whole task"
    );
    assert_eq!(
        fs::read(dst.path().join("a")).expect("read"),
        b"OLD",
        "existing dest must be untouched"
    );
    assert_eq!(
        fs::read(dst.path().join("b")).expect("read"),
        b"OLD",
        "existing dest must be untouched"
    );
}

#[test]
fn conflict_rename_suffix_handles_extensionless_and_nested_dirs() {
    let src = tmp("fc-ren2-src");
    let dst = tmp("fc-ren2-dst");
    write_file(&src.path().join("nested/blob"), b"NEW");
    write_file(&dst.path().join("blob"), b"OTHER");

    let sch = sched(1);
    let h = sch
        .submit(JobRequest {
            kind: JobKind::Copy(params_with(ConflictStrategy::RenameSuffix)),
            source: src.path().to_path_buf(),
            dest: dst.path().to_path_buf(),
        })
        .expect("submit");
    assert_eq!(
        await_terminal(&h, Duration::from_secs(30)),
        TaskState::CompleteOk
    );

    assert_eq!(fs::read(dst.path().join("blob")).expect("read"), b"OTHER");
    assert_eq!(
        fs::read(dst.path().join("nested/blob")).expect("read nested"),
        b"NEW"
    );
}
