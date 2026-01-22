use crate::config::{REPO_NAME, REPO_OWNER, SLUS_FOLDER, SPARSE_PATH};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{Emitter, Window};

/// GitHub tree entry from API response
#[derive(Debug, Deserialize)]
struct TreeEntry {
    path: String,
    #[serde(rename = "type")]
    entry_type: String,
    sha: String,
}

/// GitHub tree response
#[derive(Debug, Deserialize)]
struct TreeResponse {
    #[allow(dead_code)]
    sha: String,
    tree: Vec<TreeEntry>,
    truncated: bool,
}

/// GitHub commit response (for getting latest commit)
#[derive(Debug, Deserialize)]
struct CommitResponse {
    sha: String,
}

/// File info for sync comparison
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
pub struct FileInfo {
    pub path: String,
    pub sha: String,
}

/// Progress payload for sync events
#[derive(Clone, Serialize)]
pub struct SyncProgressPayload {
    pub stage: String,
    pub message: String,
    pub current: Option<u32>,
    pub total: Option<u32>,
}

/// Sync result summary
#[derive(Debug, Clone, Serialize)]
pub struct SyncResult {
    pub files_downloaded: u32,
    pub files_deleted: u32,
    pub files_skipped: u32,
    pub new_commit_sha: String,
}

/// Compute git blob SHA for a file (same format git uses)
fn compute_git_blob_sha(path: &Path) -> Result<String, String> {
    let content = fs::read(path).map_err(|e| format!("Failed to read file: {}", e))?;
    let header = format!("blob {}\0", content.len());

    let mut hasher = Sha1::new();
    hasher.update(header.as_bytes());
    hasher.update(&content);

    Ok(hex::encode(hasher.finalize()))
}

/// Check if a path should be skipped (user-customs folder)
fn should_skip_path(path: &str) -> bool {
    path.contains("user-customs")
}

/// Check if a filename is a disabled (dash-prefixed) version
fn is_disabled_file(filename: &str) -> bool {
    filename.starts_with('-')
}

/// Get the disabled version path for a file
fn get_disabled_path(path: &str) -> String {
    if let Some(pos) = path.rfind('/') {
        let dir = &path[..pos + 1];
        let file = &path[pos + 1..];
        format!("{}-{}", dir, file)
    } else {
        format!("-{}", path)
    }
}

/// Get the enabled version path for a disabled file
fn get_enabled_path(path: &str) -> Option<String> {
    if let Some(pos) = path.rfind("/-") {
        let dir = &path[..pos + 1];
        let file = &path[pos + 2..]; // Skip "/-"
        Some(format!("{}{}", dir, file))
    } else if path.starts_with('-') {
        Some(path[1..].to_string())
    } else {
        None
    }
}

/// Get the latest commit SHA for the main branch
#[tauri::command]
pub async fn get_latest_commit() -> Result<String, String> {
    let client = Client::new();
    let url = format!(
        "https://api.github.com/repos/{}/{}/commits/main",
        REPO_OWNER, REPO_NAME
    );

    let response = client
        .get(&url)
        .header("User-Agent", "NCAA-NEXT-Textures-Downloader")
        .header("Accept", "application/vnd.github.v3+json")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch latest commit: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "GitHub API error: {} - {}",
            response.status(),
            response.text().await.unwrap_or_default()
        ));
    }

    let commit: CommitResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse commit response: {}", e))?;

    Ok(commit.sha)
}

/// Fetch a single tree from GitHub API
async fn fetch_tree(client: &Client, tree_sha: &str, recursive: bool) -> Result<TreeResponse, String> {
    let url = if recursive {
        format!(
            "https://api.github.com/repos/{}/{}/git/trees/{}?recursive=1",
            REPO_OWNER, REPO_NAME, tree_sha
        )
    } else {
        format!(
            "https://api.github.com/repos/{}/{}/git/trees/{}",
            REPO_OWNER, REPO_NAME, tree_sha
        )
    };

    let response = client
        .get(&url)
        .header("User-Agent", "NCAA-NEXT-Textures-Downloader")
        .header("Accept", "application/vnd.github.v3+json")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch tree: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "GitHub API error: {} - {}",
            response.status(),
            response.text().await.unwrap_or_default()
        ));
    }

    response
        .json()
        .await
        .map_err(|e| format!("Failed to parse tree response: {}", e))
}

/// Navigate to a subtree by path (e.g., "textures/SLUS-21214")
async fn get_subtree_sha(client: &Client, root_sha: &str, path: &str) -> Result<String, String> {
    let parts: Vec<&str> = path.split('/').collect();
    let mut current_sha = root_sha.to_string();

    for part in parts {
        let tree = fetch_tree(client, &current_sha, false).await?;

        let entry = tree.tree.iter()
            .find(|e| e.path == part && e.entry_type == "tree")
            .ok_or_else(|| format!("Path component '{}' not found in repository", part))?;

        current_sha = entry.sha.clone();
    }

    Ok(current_sha)
}

/// Recursively fetch all files from a tree, handling truncation
async fn fetch_tree_files_recursive(
    client: &Client,
    tree_sha: &str,
    base_path: &str,
    file_map: &mut HashMap<String, String>,
) -> Result<(), String> {
    let tree = fetch_tree(client, tree_sha, true).await?;

    if tree.truncated {
        // Tree is truncated, need to fetch each subdirectory individually
        let tree_non_recursive = fetch_tree(client, tree_sha, false).await?;

        for entry in tree_non_recursive.tree {
            let entry_path = if base_path.is_empty() {
                entry.path.clone()
            } else {
                format!("{}/{}", base_path, entry.path)
            };

            if entry.entry_type == "blob" {
                file_map.insert(entry_path, entry.sha);
            } else if entry.entry_type == "tree" {
                // Recursively fetch this subdirectory
                Box::pin(fetch_tree_files_recursive(client, &entry.sha, &entry_path, file_map)).await?;
            }
        }
    } else {
        // Tree is complete, add all files
        for entry in tree.tree {
            if entry.entry_type == "blob" {
                let entry_path = if base_path.is_empty() {
                    entry.path
                } else {
                    format!("{}/{}", base_path, entry.path)
                };
                file_map.insert(entry_path, entry.sha);
            }
        }
    }

    Ok(())
}

/// Fetch the GitHub tree for the sparse path
async fn fetch_github_tree() -> Result<(HashMap<String, String>, String), String> {
    let client = Client::new();

    // First get the latest commit SHA
    let commit_sha = get_latest_commit().await?;

    // Navigate to the SPARSE_PATH subtree to avoid fetching the entire repo
    let subtree_sha = get_subtree_sha(&client, &commit_sha, SPARSE_PATH).await?;

    // Now fetch all files from this subtree
    let mut file_map: HashMap<String, String> = HashMap::new();
    fetch_tree_files_recursive(&client, &subtree_sha, "", &mut file_map).await?;

    Ok((file_map, commit_sha))
}

/// Build a map of local files (relative_path -> sha)
fn build_local_file_map(textures_dir: &Path) -> Result<HashMap<String, String>, String> {
    let slus_path = textures_dir.join(SLUS_FOLDER);
    if !slus_path.exists() {
        return Err(format!("{} folder not found", SLUS_FOLDER));
    }

    let mut file_map: HashMap<String, String> = HashMap::new();
    build_local_file_map_recursive(&slus_path, &slus_path, &mut file_map)?;
    Ok(file_map)
}

fn build_local_file_map_recursive(
    base_path: &Path,
    current_path: &Path,
    file_map: &mut HashMap<String, String>,
) -> Result<(), String> {
    let entries = fs::read_dir(current_path)
        .map_err(|e| format!("Failed to read directory: {}", e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let path = entry.path();

        // Skip hidden files
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with('.') {
                continue;
            }
        }

        if path.is_dir() {
            build_local_file_map_recursive(base_path, &path, file_map)?;
        } else if path.is_file() {
            let relative_path = path
                .strip_prefix(base_path)
                .map_err(|e| format!("Failed to get relative path: {}", e))?
                .to_string_lossy()
                .to_string();

            // Use forward slashes for consistency
            let relative_path = relative_path.replace('\\', "/");

            // Skip user-customs
            if should_skip_path(&relative_path) {
                continue;
            }

            let sha = compute_git_blob_sha(&path)?;
            file_map.insert(relative_path, sha);
        }
    }

    Ok(())
}

/// Download a file from GitHub raw content
async fn download_file(
    client: &Client,
    relative_path: &str,
    dest_path: &Path,
) -> Result<(), String> {
    let url = format!(
        "https://raw.githubusercontent.com/{}/{}/main/{}/{}",
        REPO_OWNER, REPO_NAME, SPARSE_PATH, relative_path
    );

    let response = client
        .get(&url)
        .header("User-Agent", "NCAA-NEXT-Textures-Downloader")
        .send()
        .await
        .map_err(|e| format!("Failed to download file: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "Failed to download {}: HTTP {}",
            relative_path,
            response.status()
        ));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read file content: {}", e))?;

    // Ensure parent directory exists
    if let Some(parent) = dest_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory: {}", e))?;
    }

    fs::write(dest_path, &bytes).map_err(|e| format!("Failed to write file: {}", e))?;

    Ok(())
}

/// Run the sync operation
#[tauri::command]
pub async fn run_sync(textures_dir: String, window: Window) -> Result<SyncResult, String> {
    let textures_path = PathBuf::from(&textures_dir);
    let slus_path = textures_path.join(SLUS_FOLDER);

    // Emit starting status
    let _ = window.emit(
        "sync-progress",
        SyncProgressPayload {
            stage: "fetching".to_string(),
            message: "Fetching repository information...".to_string(),
            current: None,
            total: None,
        },
    );

    // Fetch GitHub tree
    let (remote_files, commit_sha) = fetch_github_tree().await?;
    let remote_count = remote_files.len();

    let _ = window.emit(
        "sync-progress",
        SyncProgressPayload {
            stage: "scanning".to_string(),
            message: format!("Found {} files in repository", remote_count),
            current: None,
            total: None,
        },
    );

    // Build local file map
    let _ = window.emit(
        "sync-progress",
        SyncProgressPayload {
            stage: "scanning".to_string(),
            message: "Scanning local files...".to_string(),
            current: None,
            total: None,
        },
    );

    let local_files = build_local_file_map(&textures_path)?;
    let local_count = local_files.len();

    let _ = window.emit(
        "sync-progress",
        SyncProgressPayload {
            stage: "scanning".to_string(),
            message: format!("Found {} local files (excluding user-customs)", local_count),
            current: None,
            total: None,
        },
    );

    // Determine files to download (new or modified)
    let mut files_to_download: Vec<String> = Vec::new();
    let mut files_skipped: u32 = 0;

    for (path, remote_sha) in &remote_files {
        // Skip user-customs
        if should_skip_path(path) {
            files_skipped += 1;
            continue;
        }

        // Check if file exists locally
        if let Some(local_sha) = local_files.get(path) {
            if local_sha == remote_sha {
                // File unchanged
                continue;
            }
        }

        // Check if disabled version exists locally
        let disabled_path = get_disabled_path(path);
        if let Some(local_sha) = local_files.get(&disabled_path) {
            if local_sha == remote_sha {
                // Disabled version is up to date
                files_skipped += 1;
                continue;
            }
            // Disabled version exists but is outdated - download to disabled path
            files_to_download.push(disabled_path);
            continue;
        }

        files_to_download.push(path.clone());
    }

    // Determine files to delete (in local but not in remote, and not user-customs)
    let mut files_to_delete: Vec<String> = Vec::new();

    for path in local_files.keys() {
        // Skip user-customs
        if should_skip_path(path) {
            continue;
        }

        // Check if file is in remote
        if remote_files.contains_key(path) {
            continue;
        }

        // Check if this is a disabled version of a file in remote
        if is_disabled_file(path.rsplit('/').next().unwrap_or(path)) {
            if let Some(enabled_path) = get_enabled_path(path) {
                if remote_files.contains_key(&enabled_path) {
                    // This is a disabled version of a file that exists in remote - keep it
                    continue;
                }
            }
        }

        // File not in remote and not a disabled version of a remote file - delete it
        files_to_delete.push(path.clone());
    }

    let download_count = files_to_download.len() as u32;
    let delete_count = files_to_delete.len() as u32;

    // Report what we're going to do
    let _ = window.emit(
        "sync-progress",
        SyncProgressPayload {
            stage: "comparing".to_string(),
            message: format!(
                "Changes: {} files to download, {} to delete, {} up to date",
                download_count,
                delete_count,
                local_count as u32 - delete_count
            ),
            current: None,
            total: None,
        },
    );

    // Download files
    let client = Client::new();
    let mut downloaded: u32 = 0;

    for (i, path) in files_to_download.iter().enumerate() {
        let _ = window.emit(
            "sync-progress",
            SyncProgressPayload {
                stage: "downloading".to_string(),
                message: format!("Downloading: {}", path),
                current: Some(i as u32 + 1),
                total: Some(download_count),
            },
        );

        // Determine the source path for download (always use the non-disabled path)
        let source_path = if is_disabled_file(path.rsplit('/').next().unwrap_or(path)) {
            get_enabled_path(path).unwrap_or_else(|| path.clone())
        } else {
            path.clone()
        };

        let dest_path = slus_path.join(path);
        download_file(&client, &source_path, &dest_path).await?;
        downloaded += 1;
    }

    // Delete files
    let mut deleted: u32 = 0;

    for (i, path) in files_to_delete.iter().enumerate() {
        let _ = window.emit(
            "sync-progress",
            SyncProgressPayload {
                stage: "deleting".to_string(),
                message: format!("Deleting: {}", path),
                current: Some(i as u32 + 1),
                total: Some(delete_count),
            },
        );

        let file_path = slus_path.join(path);
        if file_path.exists() {
            fs::remove_file(&file_path)
                .map_err(|e| format!("Failed to delete {}: {}", path, e))?;
            deleted += 1;

            // Try to remove empty parent directories
            if let Some(parent) = file_path.parent() {
                let _ = fs::remove_dir(parent); // Ignore errors (directory not empty)
            }
        }
    }

    // Complete
    let _ = window.emit(
        "sync-progress",
        SyncProgressPayload {
            stage: "complete".to_string(),
            message: format!(
                "Sync complete! Downloaded: {}, Deleted: {}, Skipped: {}",
                downloaded, deleted, files_skipped
            ),
            current: None,
            total: None,
        },
    );

    Ok(SyncResult {
        files_downloaded: downloaded,
        files_deleted: deleted,
        files_skipped,
        new_commit_sha: commit_sha,
    })
}

/// Check sync status without making changes
#[tauri::command]
pub async fn check_sync_status(textures_dir: String) -> Result<SyncStatusResult, String> {
    let textures_path = PathBuf::from(&textures_dir);

    // Fetch GitHub tree
    let (remote_files, commit_sha) = fetch_github_tree().await?;

    // Build local file map
    let local_files = build_local_file_map(&textures_path)?;

    // Count differences
    let mut files_to_download: u32 = 0;
    let mut files_to_delete: u32 = 0;
    let mut files_up_to_date: u32 = 0;

    for (path, remote_sha) in &remote_files {
        if should_skip_path(path) {
            continue;
        }

        if let Some(local_sha) = local_files.get(path) {
            if local_sha == remote_sha {
                files_up_to_date += 1;
                continue;
            }
        }

        // Check disabled version
        let disabled_path = get_disabled_path(path);
        if let Some(local_sha) = local_files.get(&disabled_path) {
            if local_sha == remote_sha {
                files_up_to_date += 1;
                continue;
            }
        }

        files_to_download += 1;
    }

    for path in local_files.keys() {
        if should_skip_path(path) {
            continue;
        }

        if !remote_files.contains_key(path) {
            if is_disabled_file(path.rsplit('/').next().unwrap_or(path)) {
                if let Some(enabled_path) = get_enabled_path(path) {
                    if remote_files.contains_key(&enabled_path) {
                        continue;
                    }
                }
            }
            files_to_delete += 1;
        }
    }

    Ok(SyncStatusResult {
        latest_commit_sha: commit_sha,
        files_to_download,
        files_to_delete,
        files_up_to_date,
        is_up_to_date: files_to_download == 0 && files_to_delete == 0,
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncStatusResult {
    pub latest_commit_sha: String,
    pub files_to_download: u32,
    pub files_to_delete: u32,
    pub files_up_to_date: u32,
    pub is_up_to_date: bool,
}
