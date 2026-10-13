//! Linux backend: disk images (`.iso`, `.img`, `.cdr`, `.toast`) via udisks2.
//!
//! `udisksctl loop-setup` maps the image through a loop device and scans it for
//! partitions; every filesystem the mapping exposes is then mounted read-only
//! and udisks2 picks the mount point (`/run/media/<user>/<label>`). The
//! mappings are found back by their backing file, and `udisksctl loop-delete`
//! frees the device again once nothing on it is mounted.
//!
//! Because the kernel mounts whatever filesystem the image holds, `.dmg` is not
//! accepted here: it is Apple's own container format and needs `dmg2img` first.

use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use cba::broc::has;

/// Extensions [`supported`] accepts.
#[cfg(any(target_os = "linux", test))]
const EXTENSIONS: [&str; 5] = ["iso", "img", "image", "cdr", "toast"];

#[cfg(any(target_os = "linux", test))]
fn is_image_extension(path: &Path) -> bool {
    path.extension().is_some_and(|ext| {
        EXTENSIONS
            .iter()
            .any(|known| ext.eq_ignore_ascii_case(known))
    })
}

/// External tools this backend runs; a system without them cannot mount an
/// image at all.
const TOOLS: [&str; 4] = ["udisksctl", "lsblk", "findmnt", "losetup"];

fn tools_present() -> bool {
    TOOLS.iter().all(|tool| has(tool))
}

/// Whether `path` is a disk image this backend can mount: a known extension and
/// [`TOOLS`] on `PATH`.
#[cfg(target_os = "linux")]
pub fn supported(path: &Path) -> bool {
    is_image_extension(path) && tools_present()
}

#[cfg(not(target_os = "linux"))]
pub fn supported(_path: &Path) -> bool {
    false
}

/// Maps `source` and mounts every filesystem the mapping exposes, returning the
/// mount points udisks2 chose, in device order.
#[cfg(target_os = "linux")]
pub fn mount(source: &Path) -> Result<Vec<PathBuf>, String> {
    let output = run(udisksctl()
        .args(["loop-setup", "-f"])
        .arg(source)
        .args(["-r", "--no-user-interaction"]))?;

    let Some(device) = parse_loop_device(&output) else {
        return Err("udisksctl mapped nothing".to_string());
    };

    // the loop device itself when it holds a filesystem, its partitions
    // otherwise; a member without one is skipped, its error kept in case
    // nothing at all could be mounted
    let mut failure = None;
    for member in members(&device) {
        if mount_points(&member).is_empty()
            && let Err(err) = mount_member(&member)
        {
            failure = failure.or(Some(err));
        }
    }

    let points = mounted_points(&device);
    if points.is_empty() {
        return Err(failure.unwrap_or_else(|| "udisksctl mounted nothing".to_string()));
    }
    Ok(points)
}

#[cfg(not(target_os = "linux"))]
pub fn mount(_source: &Path) -> Result<Vec<PathBuf>, String> {
    Err("mounting is not supported on this platform".to_string())
}

/// Where `source` is already mounted, if it is: the answer covers mappings this
/// process never set up, which would otherwise be mapped a second time.
#[cfg(target_os = "linux")]
pub fn attached_at(source: &Path) -> Option<Vec<PathBuf>> {
    let points = loop_devices(source)
        .iter()
        .flat_map(|device| mounted_points(device))
        .collect::<Vec<PathBuf>>();

    (!points.is_empty()).then_some(points)
}

#[cfg(not(target_os = "linux"))]
pub fn attached_at(_source: &Path) -> Option<Vec<PathBuf>> {
    None
}

/// Unmounts the filesystem at `mount_point` and frees the loop device behind
/// it, which goes away only once nothing on it is mounted.
#[cfg(target_os = "linux")]
pub fn detach(mount_point: &Path) -> Result<(), String> {
    let Some(device) = mounted_device(mount_point) else {
        return Err(format!("{} is not mounted", mount_point.display()));
    };
    let device = parent_loop(&device);

    for member in members(&device) {
        if !mount_points(&member).is_empty() {
            run(udisksctl()
                .args(["unmount", "-b"])
                .arg(&member)
                .arg("--no-user-interaction"))?;
        }
    }

    run(udisksctl()
        .args(["loop-delete", "-b"])
        .arg(&device)
        .arg("--no-user-interaction"))?;
    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn detach(_mount_point: &Path) -> Result<(), String> {
    Err("unmounting is not supported on this platform".to_string())
}

/// The loop devices backing `source`, so a mapping made elsewhere is found.
#[cfg(target_os = "linux")]
fn loop_devices(source: &Path) -> Vec<PathBuf> {
    run(losetup().args(["-j"]).arg(source))
        .map(|report| parse_devices(&report))
        .unwrap_or_default()
}

/// Mounts one member of the mapping read-only through udisks2.
#[cfg(target_os = "linux")]
fn mount_member(member: &Path) -> Result<(), String> {
    run(udisksctl()
        .args(["mount", "-b"])
        .arg(member)
        .arg("--no-user-interaction"))
    .map(|_| ())
}

/// The loop device and the partitions it exposes, itself first.
#[cfg(target_os = "linux")]
fn members(device: &Path) -> Vec<PathBuf> {
    run(command("lsblk")
        .args(["-n", "-r", "-o", "NAME"])
        .arg(device))
    .map(|report| parse_members(&report))
    .unwrap_or_default()
}

/// The mount points of `device` or one of its partitions.
#[cfg(target_os = "linux")]
fn mounted_points(device: &Path) -> Vec<PathBuf> {
    let mut points = Vec::new();
    for member in members(device) {
        points.extend(mount_points(&member));
    }
    points
}

/// The mount points of one device, empty while it is not mounted.
#[cfg(target_os = "linux")]
fn mount_points(member: &Path) -> Vec<PathBuf> {
    run(command("findmnt")
        .args(["-n", "-o", "TARGET", "--source"])
        .arg(member))
    .map(|report| parse_paths(&report))
    .unwrap_or_default()
}

/// The device a mount point belongs to.
#[cfg(target_os = "linux")]
fn mounted_device(mount_point: &Path) -> Option<PathBuf> {
    let report = run(command("findmnt")
        .args(["-n", "-o", "SOURCE", "-T"])
        .arg(mount_point))
    .ok()?;
    parse_paths(&report).into_iter().next()
}

/// The loop device a member belongs to: the device itself, or the device its
/// partition sits on.
#[cfg(target_os = "linux")]
fn parent_loop(member: &Path) -> PathBuf {
    run(command("lsblk")
        .args(["-n", "-r", "-o", "NAME", "-s"])
        .arg(member))
    .map(|report| parse_members(&report))
    .ok()
    .and_then(|devices| devices.into_iter().next_back())
    .unwrap_or_else(|| member.to_path_buf())
}

/// The loop device `udisksctl loop-setup` reported: `Mapped file … as
/// /dev/loop3.`
#[cfg(any(target_os = "linux", test))]
fn parse_loop_device(report: &str) -> Option<PathBuf> {
    let device = report.split_whitespace().next_back()?.trim_end_matches('.');
    device
        .starts_with("/dev/loop")
        .then(|| PathBuf::from(device))
}

/// The devices of a `losetup -j` report, one per line.
#[cfg(any(target_os = "linux", test))]
fn parse_devices(report: &str) -> Vec<PathBuf> {
    report
        .lines()
        .filter_map(|line| line.split_once(':'))
        .map(|(device, _)| PathBuf::from(device.trim()))
        .filter(|device| device.starts_with("/dev/"))
        .collect()
}

/// The device names of an `lsblk` report, resolved below `/dev`.
#[cfg(any(target_os = "linux", test))]
fn parse_members(report: &str) -> Vec<PathBuf> {
    report
        .lines()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(|name| Path::new("/dev").join(name))
        .collect()
}

/// The paths of a line-oriented report, which is empty lines when there are
/// none.
#[cfg(any(target_os = "linux", test))]
fn parse_paths(report: &str) -> Vec<PathBuf> {
    report
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with('/'))
        .map(PathBuf::from)
        .collect()
}

/// `udisksctl` with stdin closed, so an authentication prompt cannot read the
/// terminal.
fn udisksctl() -> Command {
    command("udisksctl")
}

fn losetup() -> Command {
    command("losetup")
}

fn command(program: &str) -> Command {
    let mut command = Command::new(program);
    command.stdin(Stdio::null());
    command
}

/// Runs `command` and returns its stdout, or its first diagnostic line.
fn run(command: &mut Command) -> Result<String, String> {
    let output = command
        .output()
        .map_err(|err| format!("{}: {err}", command.get_program().to_string_lossy()))?;

    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
    Err(failure(&output))
}

/// A tool's first non-empty diagnostic line.
fn failure(output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    stderr
        .lines()
        .chain(stdout.lines())
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("the mount helper failed")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loop_setup_report_names_the_device() {
        assert_eq!(
            parse_loop_device("Mapped file /tmp/a.iso as /dev/loop3.\n"),
            Some(PathBuf::from("/dev/loop3"))
        );
        assert_eq!(
            parse_loop_device("Mapped file /tmp/My Image.img as /dev/loop12"),
            Some(PathBuf::from("/dev/loop12"))
        );
        assert_eq!(parse_loop_device(""), None);
        assert_eq!(parse_loop_device("Error: no such file"), None);
    }

    #[test]
    fn backing_devices_are_read_from_losetup() {
        assert_eq!(
            parse_devices("/dev/loop1: []: (/tmp/a.iso)\n/dev/loop0: []: (/tmp/a.iso)\n"),
            vec![PathBuf::from("/dev/loop1"), PathBuf::from("/dev/loop0")]
        );
        assert!(parse_devices("").is_empty());
    }

    #[test]
    fn device_names_are_resolved_below_dev() {
        assert_eq!(
            parse_members("loop0\nloop0p1\n\n"),
            vec![PathBuf::from("/dev/loop0"), PathBuf::from("/dev/loop0p1")]
        );
        assert!(parse_members("").is_empty());
    }

    #[test]
    fn paths_are_read_from_line_oriented_reports() {
        assert_eq!(
            parse_paths("\n/run/media/me/LABEL\n/run/media/me/OTHER\n"),
            vec![
                PathBuf::from("/run/media/me/LABEL"),
                PathBuf::from("/run/media/me/OTHER")
            ]
        );
        assert!(parse_paths("\n\n").is_empty());
    }

    #[test]
    fn image_extensions_are_recognized_case_insensitively() {
        assert!(is_image_extension(Path::new("A.iso")));
        assert!(is_image_extension(Path::new("A.IMG")));
        assert!(is_image_extension(Path::new("disc.toast")));
        assert!(!is_image_extension(Path::new("A.dmg")));
        assert!(!is_image_extension(Path::new("A.zip")));
        assert!(!is_image_extension(Path::new("README")));
    }
}
