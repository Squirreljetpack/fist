//! End-to-end extraction through the public API: detection, listing, and
//! scheduler-submitted extract jobs.
//!
//! Fixtures are built with the system archivers when available; formats
//! whose tool is missing are skipped.

use std::path::{Path, PathBuf};
use std::time::Duration;

use fist_copy::{ExtractParams, JobKind, JobRequest, Scheduler, SchedulerOptions};

fn have(cmd: &str) -> bool {
    std::process::Command::new(cmd)
        .arg("--version")
        .output()
        .is_ok()
}

struct Fixture {
    dir: PathBuf,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn fixture() -> Fixture {
    static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let uniq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("fist-copy-extract-{}-{uniq}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src/nested")).unwrap();
    std::fs::write(dir.join("src/hello.txt"), b"hello world\n").unwrap();
    std::fs::write(dir.join("src/nested/data.txt"), b"nested data\n").unwrap();
    Fixture { dir }
}

fn run(cmd: &mut std::process::Command) {
    let out = cmd.output().expect("spawn archiver");
    assert!(out.status.success(), "archiver failed: {cmd:?}");
}

/// Creates one archive per available tool; returns (path, format id).
fn make_archives(f: &Fixture) -> Vec<(PathBuf, &'static str)> {
    let mut out = Vec::new();
    let d = &f.dir;
    let src = |p: &str| d.join(p);

    if have("tar") {
        let p = d.join("a.tar");
        run(std::process::Command::new("tar")
            .args(["-cf"])
            .arg(&p)
            .arg("-C")
            .arg(src("src"))
            .args(["."]));
        out.push((p, "tar"));

        let p = d.join("a.tar.gz");
        run(std::process::Command::new("tar")
            .args(["-czf"])
            .arg(&p)
            .arg("-C")
            .arg(src("src"))
            .args(["."]));
        out.push((p, "gz"));

        // a bare gz stream (not a tar); note gzip names it <name>.txt.gz
        std::fs::copy(src("src/hello.txt"), d.join("bare.txt")).unwrap();
        run(std::process::Command::new("gzip")
            .arg("-kf")
            .arg(d.join("bare.txt")));
        out.push((d.join("bare.txt.gz"), "gz"));
    }
    if have("zip") {
        let p = d.join("a.zip");
        run(std::process::Command::new("zip")
            .args(["-q", "-r"])
            .arg(&p)
            .arg(".")
            .current_dir(src("src")));
        out.push((p, "zip"));
    }
    if have("ar") {
        let p = d.join("a.ar");
        run(std::process::Command::new("ar")
            .args(["rcs"])
            .arg(&p)
            .arg(src("src/hello.txt"))
            .arg(src("src/nested/data.txt")));
        out.push((p, "ar"));
    }
    if have("7z") {
        let p = d.join("a.7z");
        run(std::process::Command::new("7z")
            .args(["a", "-bso0", "-bsp0", "-y"])
            .arg(&p)
            .arg(".")
            .current_dir(src("src")));
        out.push((p, "7z"));
    }
    if have("rar") {
        let p = d.join("a.rar");
        run(std::process::Command::new("rar")
            .args(["a", "-r", "-idq"])
            .arg(&p)
            .arg(".")
            .current_dir(src("src")));
        out.push((p, "rar"));
    }
    out
}

/// Polls the task to a terminal state, returning whether it was ok.
fn wait_ok(handle: &fist_copy::TaskHandle) -> bool {
    for _ in 0..300 {
        let snap = handle.snapshot();
        if snap.state.is_terminal() {
            return snap.state == fist_copy::TaskState::CompleteOk;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("task did not finish");
}

#[test]
fn detect_list_extract_roundtrip() {
    let f = fixture();
    for (path, _) in make_archives(&f) {
        eprintln!("archive {} exists={}", path.display(), path.exists());
        let Some(format) = fist_copy::extract::detect(&path) else {
            panic!("undetected: {}", path.display());
        };
        let listing = fist_copy::extract::list(&path, format)
            .unwrap_or_else(|e| panic!("list {}: {e}", path.display()));
        assert!(!listing.is_empty(), "empty listing for {}", path.display());

        let dest = f.dir.join(format!(
            "out-{}",
            path.file_name().unwrap().to_str().unwrap()
        ));
        std::fs::create_dir_all(&dest).unwrap();

        let sched = Scheduler::new(SchedulerOptions {
            workers: std::num::NonZeroUsize::new(2).unwrap(),
        });
        let handle = sched
            .submit(JobRequest {
                kind: JobKind::Extract(ExtractParams),
                source: path.clone(),
                dest: dest.clone(),
            })
            .expect("submit");

        assert!(wait_ok(&handle), "extraction failed for {}", path.display());
        let snap = handle.snapshot();
        assert_eq!(snap.files_total as usize, listing.len());

        // every listed file materialized somewhere under dest
        for entry in &listing {
            if entry.is_dir {
                continue;
            }
            let candidate = dest.join(&entry.path);
            assert!(
                candidate.exists() || symlink_exists(&candidate),
                "missing extracted file {} from {}",
                candidate.display(),
                path.display()
            );
        }

        // progress denominators resolve fully
        assert_eq!(snap.files_failed, 0);
        sched.shutdown(Duration::from_secs(2));
    }
}

fn symlink_exists(p: &Path) -> bool {
    std::fs::symlink_metadata(p).is_ok()
}

#[test]
fn cancel_before_start_yields_canceled_state() {
    let f = fixture();
    let mut archives = make_archives(&f);
    let Some((path, _)) = archives.pop() else {
        return;
    };

    let dest = f.dir.join("out-cancel");
    std::fs::create_dir_all(&dest).unwrap();

    let sched = Scheduler::new(SchedulerOptions {
        workers: std::num::NonZeroUsize::new(1).unwrap(),
    });
    let handle = sched
        .submit(JobRequest {
            kind: JobKind::Extract(ExtractParams),
            source: path,
            dest,
        })
        .expect("submit");
    handle.cancel();
    assert!(!wait_ok(&handle));
    sched.shutdown(Duration::from_secs(2));
}
