use std::fs::{self, Metadata, Permissions};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use crate::log::TaskLog;
use crate::progress::Progress;
use crate::work::DirId;

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Attrs {
    #[cfg(unix)]
    pub mode: Option<u32>,
    pub atime: Option<SystemTime>,
    pub mtime: Option<SystemTime>,
}

impl Attrs {
    pub(crate) fn from_metadata(md: &Metadata) -> Self {
        Self {
            #[cfg(unix)]
            mode: {
                use std::os::unix::fs::PermissionsExt;
                Some(md.permissions().mode())
            },
            atime: md.accessed().ok(),
            mtime: md.modified().ok(),
        }
    }
}

pub(crate) fn apply_file_meta(
    dst: &Path,
    attrs: &Attrs,
    preserve: bool,
) {
    if !preserve {
        return;
    }
    #[cfg(unix)]
    if let Some(mode) = attrs.mode {
        use std::os::unix::fs::PermissionsExt;
        let res = fs::set_permissions(dst, Permissions::from_mode(mode));
        if let Err(e) = res {
            log_perm_failure(dst, e);
        }
    }
    let times_ok = (|| -> std::io::Result<()> {
        let mut f = fs::OpenOptions::new().write(true).open(dst)?;
        set_std_times(&mut f, attrs)
    })();
    if let Err(e) = times_ok {
        log_time_failure(dst, e);
    }
}

fn log_perm_failure(
    path: &Path,
    e: std::io::Error,
) {
    log::warn!(
        "fist-copy: could not preserve permissions on {}: {e}",
        path.display()
    );
}

fn log_time_failure(
    path: &Path,
    e: std::io::Error,
) {
    log::warn!(
        "fist-copy: could not preserve timestamps on {}: {e}",
        path.display()
    );
}

fn set_std_times(
    f: &mut fs::File,
    attrs: &Attrs,
) -> std::io::Result<()> {
    use std::fs::FileTimes;
    let mut t = FileTimes::new();
    if let Some(a) = attrs.atime {
        t = t.set_accessed(a);
    }
    if let Some(m) = attrs.mtime {
        t = t.set_modified(m);
    }
    f.set_times(t)
}

pub(crate) struct DirMetaTracker {
    nodes: Mutex<Vec<Node>>,
    root_done: AtomicBool,
    root_registered: AtomicBool,
    preserve_meta: bool,
    delete_source: bool,
    prog: Arc<Progress>,
    log: Arc<TaskLog>,
}

struct Node {
    parent: Option<DirId>,
    dest: PathBuf,
    src: PathBuf,
    attrs: Attrs,
    expected: usize,
    completed: usize,
    sealed: bool,
    finished: bool,
}

impl DirMetaTracker {
    pub(crate) fn new(
        preserve_meta: bool,
        delete_source: bool,
        prog: Arc<Progress>,
        log: Arc<TaskLog>,
    ) -> Self {
        Self {
            nodes: Mutex::new(Vec::new()),
            root_done: AtomicBool::new(false),
            root_registered: AtomicBool::new(false),
            preserve_meta,
            delete_source,
            prog,
            log,
        }
    }

    pub(crate) fn register_root(
        &self,
        dest: PathBuf,
        src: PathBuf,
        attrs: Attrs,
    ) {
        self.nodes.lock().expect("dir tracker poisoned").push(Node {
            parent: None,
            dest,
            src,
            attrs,
            expected: 0,
            completed: 0,
            sealed: false,
            finished: false,
        });
        self.root_registered.store(true, Ordering::Release);
    }

    pub(crate) fn register_dir(
        &self,
        parent: DirId,
        dest: PathBuf,
        src: PathBuf,
        attrs: Attrs,
    ) -> DirId {
        let mut g = self.nodes.lock().expect("dir tracker poisoned");
        g.push(Node {
            parent: Some(parent),
            dest,
            src,
            attrs,
            expected: 0,
            completed: 0,
            sealed: false,
            finished: false,
        });
        g.len() - 1
    }

    pub(crate) fn expect_child(
        &self,
        dir: DirId,
    ) {
        if let Ok(mut g) = self.nodes.lock()
            && let Some(n) = g.get_mut(dir)
        {
            n.expected += 1;
        }
    }

    pub(crate) fn seal(
        &self,
        dir: DirId,
    ) {
        let fire = {
            let mut g = self.nodes.lock().expect("dir tracker poisoned");
            match g.get_mut(dir) {
                Some(n) => {
                    n.sealed = true;
                    arm_if_complete(n)
                }
                None => false,
            }
        };
        if fire {
            self.finalize_node(dir);
        }
    }

    pub(crate) fn child_finished(
        &self,
        dir: DirId,
    ) {
        let fire = {
            let mut g = self.nodes.lock().expect("dir tracker poisoned");
            match g.get_mut(dir) {
                Some(n) => {
                    n.completed += 1;
                    arm_if_complete(n)
                }
                None => false,
            }
        };
        if fire {
            self.finalize_node(dir);
        }
    }

    fn finalize_node(
        &self,
        dir: DirId,
    ) {
        let snapshot = {
            let g = self.nodes.lock().expect("dir tracker poisoned");
            let n = &g[dir];
            (n.dest.clone(), n.attrs, n.src.clone(), n.parent)
        };
        let (dest, attrs, src, parent) = snapshot;

        if self.preserve_meta {
            apply_dir_meta(&dest, &attrs, &self.log);
        }
        if self.delete_source {
            self.prog.cleanup_started();
            match fs::remove_dir(&src) {
                Ok(()) => self.prog.cleanup_done(1),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => self.prog.cleanup_done(1),
                Err(e) => {
                    self.prog.cleanup_failed();
                    self.log.error(format!(
                        "could not remove source directory {}: {e}",
                        src.display()
                    ));
                }
            }
        }

        match parent {
            Some(p) => self.child_finished(p),
            None => self.root_done.store(true, Ordering::Release),
        }
    }

    pub(crate) fn root_finished(&self) -> bool {
        self.root_done.load(Ordering::Acquire)
    }

    pub(crate) fn has_dirs(&self) -> bool {
        self.root_registered.load(Ordering::Acquire)
    }
}

fn arm_if_complete(n: &mut Node) -> bool {
    if n.sealed && !n.finished && n.completed >= n.expected {
        n.finished = true;
        true
    } else {
        false
    }
}

fn apply_dir_meta(
    dest: &Path,
    attrs: &Attrs,
    log: &TaskLog,
) {
    #[cfg(unix)]
    if let Some(mode) = attrs.mode
        && let Err(e) = set_dir_mode(dest, mode)
    {
        log_perm_failure(dest, e);
    }
    let res = set_dir_times(dest, attrs);
    if let Err(e) = res {
        log_time_failure(dest, e);
    }
    let _ = log;
}

#[cfg(unix)]
fn set_dir_times(
    path: &Path,
    attrs: &Attrs,
) -> std::io::Result<()> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let cpath = CString::new(path.as_os_str().as_bytes())?;
    let times = [time_spec(attrs.atime), time_spec(attrs.mtime)];
    let rc = unsafe { libc::utimensat(libc::AT_FDCWD, cpath.as_ptr(), times.as_ptr(), 0) };
    if rc != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(not(unix))]
fn set_dir_times(
    _path: &Path,
    _attrs: &Attrs,
) -> std::io::Result<()> {
    Ok(())
}

#[cfg(unix)]
fn time_spec(t: Option<SystemTime>) -> libc::timespec {
    match t {
        None => libc::timespec {
            tv_sec: 0,
            tv_nsec: libc::UTIME_OMIT as libc::c_long,
        },
        Some(v) => {
            let (sec, nsec) = split_epoch(v);
            libc::timespec {
                tv_sec: sec,
                tv_nsec: nsec,
            }
        }
    }
}

#[cfg(unix)]
fn set_dir_mode(
    path: &Path,
    mode: u32,
) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, Permissions::from_mode(mode))
}

#[cfg(unix)]
fn split_epoch(t: SystemTime) -> (i64, i64) {
    match t.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(d) => (d.as_secs() as i64, d.subsec_nanos() as i64),
        Err(e) => {
            let d = e.duration();
            let secs = d.as_secs() as i64;
            let nanos = d.subsec_nanos() as i64;
            if nanos > 0 {
                (-secs - 1, 1_000_000_000 - nanos)
            } else {
                (-secs, 0)
            }
        }
    }
}
