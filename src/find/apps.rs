use crate::{abspath::AbsPath, cli::paths::__home, db::Entry};
use ignore::WalkBuilder;

#[cfg(target_os = "macos")]
pub fn collect_apps() -> Vec<Entry> {
    use crate::find::walker::build_overrides;

    let roots = [
        &format!("{}/Applications", __home().to_string_lossy()),
        "/Applications",
        "/System/Applications",
        "/System/Library/CoreServices",
        "/System/Volumes/Preboot/Cryptexes/App/System/Applications",
    ];

    let mut builder = WalkBuilder::new(roots[0]);
    for root in &roots[1..] {
        builder.add(root);
    }

    builder
        .hidden(true)
        .follow_links(false)
        .git_ignore(false)
        .git_exclude(false)
        .git_global(false)
        .overrides(build_overrides(&roots, ["!Contents", "!**/*.app/*"]).unwrap())
        .build()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_some_and(|t| t.is_dir()))
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("app"))
        .filter_map(|e| {
            let path = e.path();

            let name = std::path::Path::new(path.file_name()?.to_str()?)
                .file_stem()?
                .to_string_lossy()
                .into_owned();

            Some(Entry::new(name, AbsPath::new_unchecked(path)))
        })
        .collect()
}

#[cfg(target_os = "linux")]
pub fn collect_apps() -> Vec<Entry> {
    let dirs = [
        "/usr/share/applications",
        &format!("{}/.local/share/applications", __home().to_string_lossy()),
    ];

    let mut builder = WalkBuilder::new(dirs[0]);
    for d in &dirs[1..] {
        builder.add(d);
    }

    builder
        .hidden(false)
        .follow_links(false)
        .git_ignore(false)
        .git_exclude(false)
        .git_global(false)
        .build()
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("desktop"))
        .filter_map(|e| {
            let path = e.path();
            let content = std::fs::read_to_string(path).ok()?;

            let mut in_entry = false;
            let mut name = String::new();
            let mut exec = String::new();
            let mut nodisplay = false;
            let mut is_application = false;

            for line in content.lines() {
                let line = line.trim();

                if line.starts_with('[') {
                    if in_entry && line.ends_with(']') {
                        break;
                    } else {
                        in_entry = line == "[Desktop Entry]";
                        continue;
                    }
                }
                if !in_entry || line.is_empty() || line.starts_with('#') {
                    continue;
                }

                if let Some((k, v)) = line.split_once('=') {
                    match k {
                        "NoDisplay" if v.eq_ignore_ascii_case("true") => {
                            nodisplay = true;
                        }
                        "Type" if v.contains("Application") => {
                            is_application = true;
                        }
                        "Name" => {
                            name = v.to_string();
                        }
                        "Exec" => {
                            exec = v.to_string();
                        }
                        _ => {}
                    }
                }
            }

            if nodisplay || !is_application {
                return None;
            }
            if exec.is_empty() || name.is_empty() {
                use cli_boilerplate_automation::_log;
                _log!("Missing from {path:?}: Exec: {exec}, Name: {name}");
                return None;
            }

            let ret = Entry::new(name, AbsPath::new_unchecked(path)).cmd(exec);
            Some(ret)
        })
        .collect()
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub fn collect_apps() -> Vec<Entry> {
    // todo
    Vec::new()
}
