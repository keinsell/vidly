use std::env;
use std::path::PathBuf;
use std::sync::LazyLock;

use directories::ProjectDirs;

pub const NAME: &str = env!("CARGO_PKG_NAME");
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub static DATA_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    ProjectDirs::from("com", "keinsell", NAME)
        .expect("project data directory not found")
        .data_dir()
        .to_path_buf()
});

pub static CACHE_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    ProjectDirs::from("com", "keinsell", NAME)
        .expect("project cache directory not found")
        .cache_dir()
        .to_path_buf()
});

pub static CONFIG_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    ProjectDirs::from("com", "keinsell", NAME)
        .expect("project config directory not found")
        .config_dir()
        .to_path_buf()
});

pub static OBJECTS_DIR: LazyLock<PathBuf> =
    LazyLock::new(|| DATA_DIR.join("objects"));

pub static DATABASE_URL: LazyLock<String> = LazyLock::new(|| {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| DATA_DIR.join("vidly.db").to_string_lossy().to_string())
});
