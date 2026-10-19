// Original work © 2026 zhebuilds. AGPLv3 plus ADDITIONAL_TERMS.md.
use std::path::{Path, PathBuf};

pub const APP_DIR_NAME: &str = "TunePeerly";
pub const DATABASE_NAME: &str = "library.db";

pub fn app_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(APP_DIR_NAME)
}

pub fn database_path(app_dir: &Path) -> PathBuf {
    app_dir.join(DATABASE_NAME)
}

/// Always `%LOCALAPPDATA%\TunePeerly`. Covers live in `covers` under this directory.
pub fn cache_dir() -> PathBuf {
    let local = std::env::var_os("LOCALAPPDATA").unwrap_or_else(|| std::ffi::OsString::from("."));
    PathBuf::from(local).join(APP_DIR_NAME)
}
