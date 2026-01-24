// Configurable constants for the PS2 Textures Downloader
// Modify these values to adapt this app for other PS2 texture mod projects
// Note: Also update frontend/config.ts to match these values

/// Application title (also update in tauri.conf.json and frontend/config.ts)
#[allow(dead_code)]
pub const APP_TITLE: &str = "PS2 Textures Downloader";

/// Repository owner (GitHub username or organization)
pub const REPO_OWNER: &str = "your-github-username";

/// Name of the texture mod repository
pub const REPO_NAME: &str = "your-repo-name";

/// Full URL to the git repository
pub const REPO_URL: &str = "https://github.com/your-github-username/your-repo-name.git";

/// The target folder name (typically the PS2 game identifier like SLUS-XXXXX)
pub const SLUS_FOLDER: &str = "SLUS-XXXXX";

/// Path within the repo to sparse checkout
pub const SPARSE_PATH: &str = "textures/SLUS-XXXXX";

/// Temporary directory name used during clone
pub const TEMP_DIR_NAME: &str = "_temp_textures_repo";
