use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

/// Persistent app state
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppState {
    /// Path to the PCSX2 textures directory (parent of SLUS folder)
    pub textures_path: Option<String>,
    /// Whether initial installation has been completed
    pub initial_setup_done: bool,
    /// SHA of the last synced commit
    pub last_sync_commit: Option<String>,
}

/// Get the path to the state file
fn get_state_path(app: &AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {}", e))?;

    // Ensure directory exists
    fs::create_dir_all(&app_data_dir)
        .map_err(|e| format!("Failed to create app data directory: {}", e))?;

    Ok(app_data_dir.join("state.json"))
}

/// Load the app state from disk
#[tauri::command]
pub fn load_state(app: AppHandle) -> Result<AppState, String> {
    let state_path = get_state_path(&app)?;

    if !state_path.exists() {
        return Ok(AppState::default());
    }

    let contents = fs::read_to_string(&state_path)
        .map_err(|e| format!("Failed to read state file: {}", e))?;

    serde_json::from_str(&contents)
        .map_err(|e| format!("Failed to parse state file: {}", e))
}

/// Save the app state to disk
#[tauri::command]
pub fn save_state(app: AppHandle, state: AppState) -> Result<(), String> {
    let state_path = get_state_path(&app)?;

    let contents = serde_json::to_string_pretty(&state)
        .map_err(|e| format!("Failed to serialize state: {}", e))?;

    fs::write(&state_path, contents)
        .map_err(|e| format!("Failed to write state file: {}", e))?;

    Ok(())
}

/// Update just the textures_path in state
#[tauri::command]
pub fn set_textures_path(app: AppHandle, path: String) -> Result<(), String> {
    let mut state = load_state(app.clone())?;
    state.textures_path = Some(path);
    save_state(app, state)
}

/// Mark initial setup as complete and save the commit SHA
#[tauri::command]
pub fn mark_setup_complete(app: AppHandle, commit_sha: String) -> Result<(), String> {
    let mut state = load_state(app.clone())?;
    state.initial_setup_done = true;
    state.last_sync_commit = Some(commit_sha);
    save_state(app, state)
}

/// Update the last sync commit SHA
#[tauri::command]
pub fn update_last_sync_commit(app: AppHandle, commit_sha: String) -> Result<(), String> {
    let mut state = load_state(app.clone())?;
    state.last_sync_commit = Some(commit_sha);
    save_state(app, state)
}

/// Manually set initial_setup_done (for users who already have textures installed)
#[tauri::command]
pub fn set_initial_setup_done(app: AppHandle, done: bool) -> Result<(), String> {
    let mut state = load_state(app.clone())?;
    state.initial_setup_done = done;
    save_state(app, state)
}
