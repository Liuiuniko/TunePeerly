use std::path::{Path, PathBuf};

pub const APP_DIR_NAME: &str = "TunePeerly";
pub const DATABASE_NAME: &str = "library.db";

pub fn app_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(APP_DIR_NAME)
}

pub fn database_path(app_dir: &Path) -> PathBuf {
    app_dir.join(DATABASE_NAME)
}
