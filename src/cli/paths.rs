use cba::{bath::root_dir, bog::BogUnwrapExt, ebog, expr_as_path_fn};
use std::{env, ffi::OsString, path::PathBuf};

pub static BINARY_FULL: &str = "fist";
pub static BINARY_SHORT: &str = "fs";

// ---------------------- DIRS ----------------------
// config defaults
pub fn cache_dir() -> PathBuf {
    dirs::cache_dir()
        ._ebog("Failed to determine cache directory") // exit if failed to determine
        .join(BINARY_FULL)
}

pub fn state_dir() -> PathBuf {
    if let Some(ret) = dirs::state_dir() {
        ret.join(BINARY_FULL)
    } else {
        dirs::home_dir()
            ._ebog("Failed to determine state directory")
            .join(".local")
            .join("state")
            .join(BINARY_FULL)
    }
}
// --------------------------------
pub fn config_dir() -> PathBuf {
    if let Some(home) = dirs::home_dir() {
        let config = home.join(".config").join(BINARY_FULL);
        if config.exists() {
            return config;
        }
    };

    dirs::config_dir()
        ._ebog("Failed to determine config directory")
        .join(BINARY_FULL)
}

pub fn current_exe() -> std::ffi::OsString {
    std::env::current_exe()
        .map(OsString::from)
        .unwrap_or(BINARY_SHORT.into())
}

// preserve shell path representation
fn cwd() -> PathBuf {
    let pwd_path = env::var("PWD").map(PathBuf::from).ok();

    match env::current_dir() {
        Ok(current) => {
            if let Some(pwd) = &pwd_path {
                if let (Ok(pwd_canon), Ok(current_canon)) =
                    (pwd.canonicalize(), current.canonicalize())
                {
                    if pwd_canon == current_canon {
                        return pwd.clone(); // use logical path from PWD
                    }
                }
            }
            current
        }
        Err(e) => {
            // fallback: walk up PWD until valid
            if let Some(mut path) = pwd_path {
                while !path.exists() {
                    if !path.pop() {
                        break;
                    }
                }
                eprintln!(
                    "Warning: current_dir() failed, using closest existing parent of PWD: {}",
                    path.display()
                );
                path
            } else {
                ebog!("{e}");
                std::process::exit(1);
            }
        }
    }
}

// the absolute current directory AT INITIALIZATION
expr_as_path_fn!(__cwd, cwd());

// the absolute home directory, or root
expr_as_path_fn!(__home, dirs::home_dir().unwrap_or(root_dir()));

// ---------------------- FILES ----------------------
#[cfg(debug_assertions)]
expr_as_path_fn!(mm_cfg_path, config_dir().join("mm.dev.toml"));
#[cfg(not(debug_assertions))]
expr_as_path_fn!(mm_cfg_path, config_dir().join("mm.toml"));

#[cfg(debug_assertions)]
expr_as_path_fn!(config_path, config_dir().join("dev.toml"));
#[cfg(not(debug_assertions))]
expr_as_path_fn!(config_path, config_dir().join("config.toml"));

#[cfg(debug_assertions)]
expr_as_path_fn!(
    lessfilter_cfg_path,
    config_dir().join("lessfilter.dev.toml")
);
#[cfg(not(debug_assertions))]
expr_as_path_fn!(lessfilter_cfg_path, config_dir().join("lessfilter.toml"));

// ---------- previewer scripts -----------
expr_as_path_fn!(liza_path, cache_dir().join("liza"));
// renders text. Also pages the output if stdout is /dev/tty for convenience.
expr_as_path_fn!(text_renderer_path, cache_dir().join("pager"));
expr_as_path_fn!(show_error_path, cache_dir().join("fist_show_error"));
