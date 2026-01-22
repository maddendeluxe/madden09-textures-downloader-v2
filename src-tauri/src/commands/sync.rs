use crate::config::{REPO_NAME, REPO_OWNER, SLUS_FOLDER, SPARSE_PATH};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{Emitter, Window};

/// GitHub tree entry from API response
#[derive(Debug, Deserialize, Clone)]
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
    commit: CommitDetails,
}

#[derive(Debug, Deserialize)]
struct CommitDetails {
    committer: CommitAuthor,
}

#[derive(Debug, Deserialize)]
struct CommitAuthor {
    date: String,
}

/// GitHub compare response
#[derive(Debug, Deserialize)]
struct CompareResponse {
    files: Option<Vec<CompareFile>>,
}

/// File entry in compare response
#[derive(Debug, Deserialize, Clone)]
struct CompareFile {
    filename: String,
    status: String, // "added", "modified", "removed", "renamed"
    previous_filename: Option<String>,
    #[allow(dead_code)]
    sha: Option<String>,
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
    pub files_renamed: u32,
    pub files_skipped: u32,
    pub new_commit_sha: String,
}

/// Verification scan result (discrepancies found)
#[derive(Debug, Clone, Serialize)]
pub struct VerificationResult {
    pub files_to_download: Vec<VerificationFile>,
    pub files_to_delete: Vec<String>,
    pub has_discrepancies: bool,
}

/// File that needs to be downloaded during verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationFile {
    pub path: String,
    pub to_disabled: bool,
}

/// Check if content is likely a text file (no null bytes in first 8KB)
fn is_text_content(content: &[u8]) -> bool {
    let check_len = content.len().min(8192);
    !content[..check_len].contains(&0)
}

/// Compute git blob SHA for a file (same format git uses)
/// Normalizes CRLF to LF for text files to match git's stored format
fn compute_git_blob_sha(path: &Path) -> Result<String, String> {
    let content = fs::read(path).map_err(|e| format!("Failed to read file: {}", e))?;

    // For text files, normalize CRLF to LF (git stores with LF, Windows checkout converts to CRLF)
    let content = if is_text_content(&content) {
        // Replace CRLF with LF
        let mut normalized = Vec::with_capacity(content.len());
        let mut i = 0;
        while i < content.len() {
            if i + 1 < content.len() && content[i] == b'\r' && content[i + 1] == b'\n' {
                normalized.push(b'\n');
                i += 2;
            } else {
                normalized.push(content[i]);
                i += 1;
            }
        }
        normalized
    } else {
        content
    };

    let header = format!("blob {}\0", content.len());

    let mut hasher = Sha1::new();
    hasher.update(header.as_bytes());
    hasher.update(&content);

    Ok(hex::encode(hasher.finalize()))
}

/// Check if a filename is a junk file that can be safely deleted during cleanup
fn is_junk_file(name: &str) -> bool {
    // All hidden files (starting with .)
    if name.starts_with('.') {
        return true;
    }
    // Windows junk files
    if name.eq_ignore_ascii_case("Thumbs.db") || name.eq_ignore_ascii_case("desktop.ini") || name.eq_ignore_ascii_case("ehthumbs.db") {
        return true;
    }
    false
}

/// Recursively remove empty directories (and OS junk files)
/// Does not remove the root directory itself, only empty subdirectories
fn cleanup_empty_directories(root: &Path, window: &Window) -> u32 {
    cleanup_empty_directories_recursive(root, true, window)
}

fn cleanup_empty_directories_recursive(dir: &Path, is_root: bool, window: &Window) -> u32 {
    let mut removed = 0;

    if !dir.is_dir() {
        return 0;
    }

    // Get all entries in this directory
    let entries: Vec<_> = match fs::read_dir(dir) {
        Ok(rd) => rd.filter_map(|e| e.ok()).collect(),
        Err(e) => {
            let _ = window.emit("sync-progress", SyncProgressPayload {
                stage: "cleanup".to_string(),
                message: format!("Error reading dir {:?}: {}", dir, e),
                current: None,
                total: None,
            });
            return 0;
        }
    };

    // First, recurse into subdirectories
    for entry in &entries {
        let path = entry.path();
        if path.is_dir() {
            removed += cleanup_empty_directories_recursive(&path, false, window);
        }
    }

    // Don't remove the root directory itself
    if is_root {
        return removed;
    }

    // Remove hidden/junk files from this directory
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_file() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if is_junk_file(name) {
                        let _ = fs::remove_file(&path);
                    }
                }
            }
        }
    }

    // Now check if this directory is empty (re-read after junk file removal)
    let remaining: Vec<_> = match fs::read_dir(dir) {
        Ok(rd) => rd.filter_map(|e| e.ok()).collect(),
        Err(_) => return removed,
    };

    if remaining.is_empty() {
        match fs::remove_dir(dir) {
            Ok(_) => {
                removed += 1;
            }
            Err(e) => {
                let _ = window.emit("sync-progress", SyncProgressPayload {
                    stage: "cleanup".to_string(),
                    message: format!("Failed to remove {:?}: {}", dir, e),
                    current: None,
                    total: None,
                });
            }
        }
    }

    removed
}

/// Check if a path should be skipped (user-customs folder or hidden files)
fn should_skip_path(path: &str) -> bool {
    // Skip user-customs folder
    if path.contains("user-customs") {
        return true;
    }
    // Skip hidden files/directories (starting with .)
    for component in path.split('/') {
        if component.starts_with('.') {
            return true;
        }
    }
    false
}

/// Check if a filename is a disabled (dash-prefixed) version
fn is_disabled_filename(filename: &str) -> bool {
    filename.starts_with('-')
}

/// Get just the filename from a path
fn get_filename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
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
    let filename = get_filename(path);
    if !is_disabled_filename(filename) {
        return None;
    }

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

/// Build request with optional auth token
fn build_request(client: &Client, url: &str, token: &Option<String>) -> reqwest::RequestBuilder {
    let mut req = client
        .get(url)
        .header("User-Agent", "NCAA-NEXT-Textures-Downloader")
        .header("Accept", "application/vnd.github.v3+json");

    if let Some(t) = token {
        req = req.header("Authorization", format!("Bearer {}", t));
    }

    req
}

/// Get the latest commit SHA for the main branch
#[tauri::command]
pub async fn get_latest_commit() -> Result<String, String> {
    get_latest_commit_with_token(&None).await
}

async fn get_latest_commit_with_token(token: &Option<String>) -> Result<String, String> {
    let (sha, _) = get_commit_details_with_token("main", token).await?;
    Ok(sha)
}

/// Fetch commit details (sha and date) for a given commit reference
async fn get_commit_details_with_token(commit_ref: &str, token: &Option<String>) -> Result<(String, String), String> {
    let client = Client::new();
    let url = format!(
        "https://api.github.com/repos/{}/{}/commits/{}",
        REPO_OWNER, REPO_NAME, commit_ref
    );

    let response = build_request(&client, &url, token)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch commit: {}", e))?;

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

    Ok((commit.sha, commit.commit.committer.date))
}

/// Fetch a single tree from GitHub API
async fn fetch_tree(client: &Client, tree_sha: &str, recursive: bool, token: &Option<String>) -> Result<TreeResponse, String> {
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

    let response = build_request(client, &url, token)
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
async fn get_subtree_sha(client: &Client, root_sha: &str, path: &str, token: &Option<String>) -> Result<String, String> {
    let parts: Vec<&str> = path.split('/').collect();
    let mut current_sha = root_sha.to_string();

    for part in parts {
        let tree = fetch_tree(client, &current_sha, false, token).await?;

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
    token: &Option<String>,
) -> Result<(), String> {
    let tree = fetch_tree(client, tree_sha, true, token).await?;

    if tree.truncated {
        // Tree is truncated, need to fetch each subdirectory individually
        let tree_non_recursive = fetch_tree(client, tree_sha, false, token).await?;

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
                Box::pin(fetch_tree_files_recursive(client, &entry.sha, &entry_path, file_map, token)).await?;
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

/// Fetch the GitHub tree for the sparse path (used for full sync)
async fn fetch_github_tree(token: &Option<String>) -> Result<(HashMap<String, String>, String), String> {
    let client = Client::new();

    // First get the latest commit SHA
    let commit_sha = get_latest_commit_with_token(token).await?;

    // Navigate to the SPARSE_PATH subtree to avoid fetching the entire repo
    let subtree_sha = get_subtree_sha(&client, &commit_sha, SPARSE_PATH, token).await?;

    // Now fetch all files from this subtree
    let mut file_map: HashMap<String, String> = HashMap::new();
    fetch_tree_files_recursive(&client, &subtree_sha, "", &mut file_map, token).await?;

    Ok((file_map, commit_sha))
}

/// GitHub Compare API file limit
const GITHUB_COMPARE_FILE_LIMIT: usize = 300;

/// Fetch changed files between two commits using compare API
/// Returns (files, is_truncated) - truncated if exactly 300 files returned
async fn fetch_changed_files(
    base_sha: &str,
    head_sha: &str,
    token: &Option<String>,
) -> Result<(Vec<CompareFile>, bool), String> {
    let client = Client::new();
    let url = format!(
        "https://api.github.com/repos/{}/{}/compare/{}...{}",
        REPO_OWNER, REPO_NAME, base_sha, head_sha
    );

    let response = build_request(&client, &url, token)
        .send()
        .await
        .map_err(|e| format!("Failed to compare commits: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "GitHub API error: {} - {}",
            response.status(),
            response.text().await.unwrap_or_default()
        ));
    }

    let compare: CompareResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse compare response: {}", e))?;

    let files = compare.files.unwrap_or_default();
    let is_truncated = files.len() >= GITHUB_COMPARE_FILE_LIMIT;

    Ok((files, is_truncated))
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
    token: &Option<String>,
) -> Result<(), String> {
    let url = format!(
        "https://raw.githubusercontent.com/{}/{}/main/{}/{}",
        REPO_OWNER, REPO_NAME, SPARSE_PATH, relative_path
    );

    let mut req = client
        .get(&url)
        .header("User-Agent", "NCAA-NEXT-Textures-Downloader");

    if let Some(t) = token {
        req = req.header("Authorization", format!("Bearer {}", t));
    }

    let response = req
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

/// Check if a local file exists (either normal or disabled version)
/// Returns (exists, is_disabled, actual_path)
fn find_local_file(slus_path: &Path, relative_path: &str) -> (bool, bool, PathBuf) {
    let normal_path = slus_path.join(relative_path);
    if normal_path.exists() {
        return (true, false, normal_path);
    }

    let disabled_path = slus_path.join(get_disabled_path(relative_path));
    if disabled_path.exists() {
        return (true, true, disabled_path);
    }

    (false, false, normal_path)
}

/// Run incremental sync (only changes since last sync)
async fn run_incremental_sync(
    textures_dir: &str,
    last_commit: &str,
    token: &Option<String>,
    window: &Window,
) -> Result<SyncResult, String> {
    let textures_path = PathBuf::from(textures_dir);
    let slus_path = textures_path.join(SLUS_FOLDER);
    let client = Client::new();

    let _ = window.emit("sync-progress", SyncProgressPayload {
        stage: "fetching".to_string(),
        message: "Fetching changes since last sync...".to_string(),
        current: None,
        total: None,
    });

    // Get latest commit
    let latest_sha = get_latest_commit_with_token(token).await?;

    if latest_sha == last_commit {
        let _ = window.emit("sync-progress", SyncProgressPayload {
            stage: "complete".to_string(),
            message: "Already up to date!".to_string(),
            current: None,
            total: None,
        });
        return Ok(SyncResult {
            files_downloaded: 0,
            files_deleted: 0,
            files_renamed: 0,
            files_skipped: 0,
            new_commit_sha: latest_sha,
        });
    }

    // Get changed files
    let (changed_files, is_truncated) = fetch_changed_files(last_commit, &latest_sha, token).await?;

    // If the response is truncated (300+ files), fall back to full sync
    if is_truncated {
        return Err("TRUNCATED: Too many changed files, falling back to full sync".to_string());
    }

    // Filter to only files in our sparse path
    let prefix = format!("{}/", SPARSE_PATH);
    let relevant_files: Vec<CompareFile> = changed_files
        .into_iter()
        .filter(|f| f.filename.starts_with(&prefix) && !should_skip_path(&f.filename))
        .collect();

    let total = relevant_files.len() as u32;
    let _ = window.emit("sync-progress", SyncProgressPayload {
        stage: "comparing".to_string(),
        message: format!("Found {} changed files", total),
        current: None,
        total: None,
    });

    let mut downloaded: u32 = 0;
    let mut deleted: u32 = 0;
    let mut renamed: u32 = 0;
    let mut skipped: u32 = 0;

    for (i, file) in relevant_files.iter().enumerate() {
        let relative_path = file.filename.strip_prefix(&prefix).unwrap().to_string();

        let _ = window.emit("sync-progress", SyncProgressPayload {
            stage: "syncing".to_string(),
            message: format!("[{}] {}", file.status, relative_path),
            current: Some(i as u32 + 1),
            total: Some(total),
        });

        match file.status.as_str() {
            "added" | "modified" => {
                // Check if we have a disabled version locally
                let (exists, is_disabled, local_path) = find_local_file(&slus_path, &relative_path);

                if exists && is_disabled {
                    // Download to the disabled path (preserve disabled state)
                    let disabled_rel_path = get_disabled_path(&relative_path);
                    let dest = slus_path.join(&disabled_rel_path);
                    download_file(&client, &relative_path, &dest, token).await?;
                } else {
                    // Download to normal path
                    download_file(&client, &relative_path, &local_path, token).await?;
                }
                downloaded += 1;
            }
            "removed" => {
                // Delete the file (check both normal and disabled versions)
                let (exists, _, local_path) = find_local_file(&slus_path, &relative_path);
                if exists {
                    fs::remove_file(&local_path)
                        .map_err(|e| format!("Failed to delete {}: {}", relative_path, e))?;
                    deleted += 1;

                    // Try to remove empty parent directories
                    if let Some(parent) = local_path.parent() {
                        let _ = fs::remove_dir(parent);
                    }
                }
            }
            "renamed" => {
                if let Some(old_filename) = &file.previous_filename {
                    if old_filename.starts_with(&prefix) {
                        let old_rel_path = old_filename.strip_prefix(&prefix).unwrap();
                        let (exists, is_disabled, old_local_path) = find_local_file(&slus_path, old_rel_path);

                        if exists {
                            // Determine new path (preserve disabled state)
                            let new_local_path = if is_disabled {
                                slus_path.join(get_disabled_path(&relative_path))
                            } else {
                                slus_path.join(&relative_path)
                            };

                            // Ensure parent directory exists
                            if let Some(parent) = new_local_path.parent() {
                                fs::create_dir_all(parent)
                                    .map_err(|e| format!("Failed to create directory: {}", e))?;
                            }

                            // Move the file
                            fs::rename(&old_local_path, &new_local_path)
                                .map_err(|e| format!("Failed to rename {}: {}", old_rel_path, e))?;
                            renamed += 1;

                            // Try to remove empty old parent directories
                            if let Some(parent) = old_local_path.parent() {
                                let _ = fs::remove_dir(parent);
                            }
                        } else {
                            // Old file doesn't exist locally, download the new one
                            let dest = slus_path.join(&relative_path);
                            download_file(&client, &relative_path, &dest, token).await?;
                            downloaded += 1;
                        }
                    }
                }
            }
            _ => {
                skipped += 1;
            }
        }
    }

    let _ = window.emit("sync-progress", SyncProgressPayload {
        stage: "complete".to_string(),
        message: format!(
            "Sync complete! Downloaded: {}, Deleted: {}, Renamed: {}, Skipped: {}",
            downloaded, deleted, renamed, skipped
        ),
        current: None,
        total: None,
    });

    Ok(SyncResult {
        files_downloaded: downloaded,
        files_deleted: deleted,
        files_renamed: renamed,
        files_skipped: skipped,
        new_commit_sha: latest_sha,
    })
}

/// Run full sync (compare all files)
async fn run_full_sync(
    textures_dir: &str,
    token: &Option<String>,
    window: &Window,
) -> Result<SyncResult, String> {
    let textures_path = PathBuf::from(textures_dir);
    let slus_path = textures_path.join(SLUS_FOLDER);

    let _ = window.emit("sync-progress", SyncProgressPayload {
        stage: "fetching".to_string(),
        message: "Fetching repository tree (this may take a while)...".to_string(),
        current: None,
        total: None,
    });

    // Fetch GitHub tree
    let (remote_files, commit_sha) = fetch_github_tree(token).await?;
    let remote_count = remote_files.len();

    let _ = window.emit("sync-progress", SyncProgressPayload {
        stage: "scanning".to_string(),
        message: format!("Found {} files in repository", remote_count),
        current: None,
        total: None,
    });

    // Build local file map
    let _ = window.emit("sync-progress", SyncProgressPayload {
        stage: "scanning".to_string(),
        message: "Scanning local files...".to_string(),
        current: None,
        total: None,
    });

    let local_files = build_local_file_map(&textures_path)?;

    let _ = window.emit("sync-progress", SyncProgressPayload {
        stage: "scanning".to_string(),
        message: format!("Found {} local files (excluding user-customs)", local_files.len()),
        current: None,
        total: None,
    });

    // Determine files to download (new or modified)
    let mut files_to_download: Vec<(String, bool)> = Vec::new(); // (path, is_disabled)

    for (path, remote_sha) in &remote_files {
        if should_skip_path(path) {
            continue;
        }

        // Check normal path
        if let Some(local_sha) = local_files.get(path) {
            if local_sha == remote_sha {
                continue; // Up to date
            }
            files_to_download.push((path.clone(), false));
            continue;
        }

        // Check disabled version
        let disabled_path = get_disabled_path(path);
        if let Some(local_sha) = local_files.get(&disabled_path) {
            if local_sha == remote_sha {
                continue; // Up to date (disabled)
            }
            files_to_download.push((path.clone(), true)); // Download to disabled path
            continue;
        }

        // File doesn't exist locally
        files_to_download.push((path.clone(), false));
    }

    // Determine files to delete (in local but not in remote)
    let mut files_to_delete: Vec<String> = Vec::new();

    for local_path in local_files.keys() {
        if should_skip_path(local_path) {
            continue;
        }

        // Get the "enabled" version of this path for comparison
        let compare_path = if is_disabled_filename(get_filename(local_path)) {
            get_enabled_path(local_path).unwrap_or_else(|| local_path.clone())
        } else {
            local_path.clone()
        };

        // If the enabled version doesn't exist in remote, delete the local file
        if !remote_files.contains_key(&compare_path) {
            files_to_delete.push(local_path.clone());
        }
    }

    let download_count = files_to_download.len() as u32;
    let delete_count = files_to_delete.len() as u32;

    let _ = window.emit("sync-progress", SyncProgressPayload {
        stage: "comparing".to_string(),
        message: format!("Changes: {} to download, {} to delete", download_count, delete_count),
        current: None,
        total: None,
    });

    // Download files
    let client = Client::new();
    let mut downloaded: u32 = 0;

    for (i, (path, is_disabled)) in files_to_download.iter().enumerate() {
        let _ = window.emit("sync-progress", SyncProgressPayload {
            stage: "downloading".to_string(),
            message: format!("Downloading: {}", path),
            current: Some(i as u32 + 1),
            total: Some(download_count),
        });

        let dest_path = if *is_disabled {
            slus_path.join(get_disabled_path(path))
        } else {
            slus_path.join(path)
        };

        download_file(&client, path, &dest_path, token).await?;
        downloaded += 1;
    }

    // Delete files
    let mut deleted: u32 = 0;

    for (i, path) in files_to_delete.iter().enumerate() {
        let _ = window.emit("sync-progress", SyncProgressPayload {
            stage: "deleting".to_string(),
            message: format!("Deleting: {}", path),
            current: Some(i as u32 + 1),
            total: Some(delete_count),
        });

        let file_path = slus_path.join(path);
        if file_path.exists() {
            fs::remove_file(&file_path)
                .map_err(|e| format!("Failed to delete {}: {}", path, e))?;
            deleted += 1;

            if let Some(parent) = file_path.parent() {
                let _ = fs::remove_dir(parent);
            }
        }
    }

    let _ = window.emit("sync-progress", SyncProgressPayload {
        stage: "complete".to_string(),
        message: format!("Sync complete! Downloaded: {}, Deleted: {}", downloaded, deleted),
        current: None,
        total: None,
    });

    Ok(SyncResult {
        files_downloaded: downloaded,
        files_deleted: deleted,
        files_renamed: 0,
        files_skipped: 0,
        new_commit_sha: commit_sha,
    })
}

/// Run post-sync verification scan to find discrepancies (does NOT fix them)
#[tauri::command]
pub async fn run_verification_scan(
    textures_dir: String,
    github_token: Option<String>,
    window: Window,
) -> Result<VerificationResult, String> {
    let textures_path = PathBuf::from(&textures_dir);

    let _ = window.emit("sync-progress", SyncProgressPayload {
        stage: "verifying".to_string(),
        message: "Running verification scan...".to_string(),
        current: None,
        total: None,
    });

    // Fetch full repo tree
    let (remote_files, _) = fetch_github_tree(&github_token).await?;

    // Build local file map (with hashes)
    let local_files = build_local_file_map(&textures_path)?;

    let _ = window.emit("sync-progress", SyncProgressPayload {
        stage: "verifying".to_string(),
        message: format!("Comparing {} local files against {} repo files...", local_files.len(), remote_files.len()),
        current: None,
        total: None,
    });

    // Find files that need to be downloaded (missing or hash mismatch)
    let mut files_to_download: Vec<VerificationFile> = Vec::new();

    for (repo_path, repo_sha) in &remote_files {
        if should_skip_path(repo_path) {
            continue;
        }

        // Check if normal version exists and matches
        if let Some(local_sha) = local_files.get(repo_path) {
            if local_sha == repo_sha {
                continue; // File exists and matches
            }
            // Hash mismatch - need to re-download
            files_to_download.push(VerificationFile {
                path: repo_path.clone(),
                to_disabled: false,
            });
            continue;
        }

        // Check if disabled version exists and matches
        let disabled_path = get_disabled_path(repo_path);
        if let Some(local_sha) = local_files.get(&disabled_path) {
            if local_sha == repo_sha {
                continue; // Disabled version exists and matches
            }
            // Disabled version has wrong hash - re-download to disabled path
            files_to_download.push(VerificationFile {
                path: repo_path.clone(),
                to_disabled: true,
            });
            continue;
        }

        // File doesn't exist locally at all
        files_to_download.push(VerificationFile {
            path: repo_path.clone(),
            to_disabled: false,
        });
    }

    // Find files that need to be deleted (local but not in repo)
    let mut files_to_delete: Vec<String> = Vec::new();

    for local_path in local_files.keys() {
        if should_skip_path(local_path) {
            continue;
        }

        // Get the "enabled" version of this path for comparison
        let compare_path = if is_disabled_filename(get_filename(local_path)) {
            get_enabled_path(local_path).unwrap_or_else(|| local_path.clone())
        } else {
            local_path.clone()
        };

        // If the enabled version doesn't exist in remote, delete the local file
        if !remote_files.contains_key(&compare_path) {
            files_to_delete.push(local_path.clone());
        }
    }

    let has_discrepancies = !files_to_download.is_empty() || !files_to_delete.is_empty();

    let _ = window.emit("sync-progress", SyncProgressPayload {
        stage: "verifying".to_string(),
        message: if has_discrepancies {
            format!("Found {} files to download, {} files to delete", files_to_download.len(), files_to_delete.len())
        } else {
            "Verification complete - no discrepancies found!".to_string()
        },
        current: None,
        total: None,
    });

    Ok(VerificationResult {
        files_to_download,
        files_to_delete,
        has_discrepancies,
    })
}

/// Apply verification fixes after user approval
#[tauri::command]
pub async fn apply_verification_fixes(
    textures_dir: String,
    files_to_download: Vec<VerificationFile>,
    files_to_delete: Vec<String>,
    github_token: Option<String>,
    window: Window,
) -> Result<(u32, u32), String> {
    let textures_path = PathBuf::from(&textures_dir);
    let slus_path = textures_path.join(SLUS_FOLDER);
    let client = Client::new();

    let mut downloaded: u32 = 0;
    let mut deleted: u32 = 0;

    // Download missing/mismatched files
    if !files_to_download.is_empty() {
        let total = files_to_download.len() as u32;
        let _ = window.emit("sync-progress", SyncProgressPayload {
            stage: "verifying".to_string(),
            message: format!("Downloading {} files...", total),
            current: None,
            total: None,
        });

        for (i, file) in files_to_download.iter().enumerate() {
            let _ = window.emit("sync-progress", SyncProgressPayload {
                stage: "verifying".to_string(),
                message: format!("Downloading: {}", file.path),
                current: Some(i as u32 + 1),
                total: Some(total),
            });

            let dest_path = if file.to_disabled {
                slus_path.join(get_disabled_path(&file.path))
            } else {
                slus_path.join(&file.path)
            };

            download_file(&client, &file.path, &dest_path, &github_token).await?;
            downloaded += 1;
        }
    }

    // Delete orphaned files
    if !files_to_delete.is_empty() {
        let total = files_to_delete.len() as u32;
        for (i, path) in files_to_delete.iter().enumerate() {
            let _ = window.emit("sync-progress", SyncProgressPayload {
                stage: "verifying".to_string(),
                message: format!("Deleting: {}", path),
                current: Some(i as u32 + 1),
                total: Some(total),
            });

            let file_path = slus_path.join(path);
            if file_path.exists() {
                fs::remove_file(&file_path)
                    .map_err(|e| format!("Failed to delete {}: {}", path, e))?;
                deleted += 1;

                if let Some(parent) = file_path.parent() {
                    let _ = fs::remove_dir(parent);
                }
            }
        }
    }

    // Clean up empty directories
    let _ = window.emit("sync-progress", SyncProgressPayload {
        stage: "verifying".to_string(),
        message: "Cleaning up empty directories...".to_string(),
        current: None,
        total: None,
    });

    let dirs_removed = cleanup_empty_directories(&slus_path, &window);
    let _ = window.emit("sync-progress", SyncProgressPayload {
        stage: "verifying".to_string(),
        message: format!("Removed {} empty directories", dirs_removed),
        current: None,
        total: None,
    });

    let _ = window.emit("sync-progress", SyncProgressPayload {
        stage: "complete".to_string(),
        message: format!("Verification fixes applied! Downloaded: {}, Deleted: {}", downloaded, deleted),
        current: None,
        total: None,
    });

    Ok((downloaded, deleted))
}

/// Run the sync operation (does NOT run verification - call run_verification_scan separately)
#[tauri::command]
pub async fn run_sync(
    textures_dir: String,
    last_sync_commit: Option<String>,
    github_token: Option<String>,
    full_sync: bool,
    window: Window,
) -> Result<SyncResult, String> {
    let result = if full_sync || last_sync_commit.is_none() {
        run_full_sync(&textures_dir, &github_token, &window).await?
    } else {
        // Try incremental sync, fall back to full sync if it fails (e.g., commit not found or too many changes)
        match run_incremental_sync(&textures_dir, last_sync_commit.as_ref().unwrap(), &github_token, &window).await {
            Ok(r) => r,
            Err(e) if e.contains("404") || e.contains("Not Found") => {
                let _ = window.emit("sync-progress", SyncProgressPayload {
                    stage: "fetching".to_string(),
                    message: "Previous sync commit not found, running full sync...".to_string(),
                    current: None,
                    total: None,
                });
                run_full_sync(&textures_dir, &github_token, &window).await?
            }
            Err(e) if e.contains("TRUNCATED") => {
                let _ = window.emit("sync-progress", SyncProgressPayload {
                    stage: "fetching".to_string(),
                    message: "Too many changes since last sync (300+), running full sync...".to_string(),
                    current: None,
                    total: None,
                });
                run_full_sync(&textures_dir, &github_token, &window).await?
            }
            Err(e) => return Err(e),
        }
    };

    // Clean up empty directories
    let textures_path = PathBuf::from(&textures_dir);
    let slus_path = textures_path.join(SLUS_FOLDER);

    let _ = window.emit("sync-progress", SyncProgressPayload {
        stage: "sync_complete".to_string(),
        message: "Cleaning up empty directories...".to_string(),
        current: None,
        total: None,
    });

    let dirs_removed = cleanup_empty_directories(&slus_path, &window);
    let _ = window.emit("sync-progress", SyncProgressPayload {
        stage: "sync_complete".to_string(),
        message: format!("Removed {} empty directories", dirs_removed),
        current: None,
        total: None,
    });

    // Sync portion complete - verification will be triggered separately by frontend
    let _ = window.emit("sync-progress", SyncProgressPayload {
        stage: "sync_complete".to_string(),
        message: format!(
            "Sync complete! Downloaded: {}, Deleted: {}, Renamed: {}. Running verification...",
            result.files_downloaded, result.files_deleted, result.files_renamed
        ),
        current: None,
        total: None,
    });

    Ok(result)
}

/// Check sync status without making changes
#[tauri::command]
pub async fn check_sync_status(
    _textures_dir: String,
    last_sync_commit: Option<String>,
    github_token: Option<String>,
) -> Result<SyncStatusResult, String> {
    // Get latest commit details
    let (latest_sha, latest_date) = get_commit_details_with_token("main", &github_token).await?;

    let has_changes = match &last_sync_commit {
        Some(last) if last == &latest_sha => false,
        _ => true,
    };

    Ok(SyncStatusResult {
        latest_commit_sha: latest_sha,
        latest_commit_date: latest_date,
        last_sync_commit,
        has_changes,
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncStatusResult {
    pub latest_commit_sha: String,
    pub latest_commit_date: String,
    pub last_sync_commit: Option<String>,
    pub has_changes: bool,
}
