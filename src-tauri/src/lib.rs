mod commands;
mod config;

use commands::{
    backup_existing_folder, check_existing_folder, check_git_installed, cleanup_processes,
    delete_existing_folder, get_git_error, start_installation, validate_directory,
    // State management
    load_state, save_state, set_textures_path, mark_setup_complete,
    update_last_sync_commit, set_initial_setup_done, set_github_token,
    set_sync_disclaimer_acknowledged,
    // Sync
    get_latest_commit, run_sync, check_sync_status,
    run_verification_scan, apply_verification_fixes, run_quick_count_check,
    analyze_full_sync, execute_analyzed_sync,
    // App info
    get_app_version, fetch_installer_data, compare_versions,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            check_existing_folder,
            backup_existing_folder,
            delete_existing_folder,
            validate_directory,
            check_git_installed,
            get_git_error,
            start_installation,
            // State management
            load_state,
            save_state,
            set_textures_path,
            mark_setup_complete,
            update_last_sync_commit,
            set_initial_setup_done,
            set_github_token,
            set_sync_disclaimer_acknowledged,
            // Sync
            get_latest_commit,
            run_sync,
            check_sync_status,
            run_verification_scan,
            apply_verification_fixes,
            run_quick_count_check,
            analyze_full_sync,
            execute_analyzed_sync,
            // App info
            get_app_version,
            fetch_installer_data,
            compare_versions,
        ])
        .on_window_event(|_window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                // Kill any running git processes when window is closed
                cleanup_processes();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
