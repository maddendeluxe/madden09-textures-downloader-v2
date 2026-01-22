mod commands;
mod config;

use commands::{
    backup_existing_folder, check_existing_folder, check_git_installed, cleanup_processes,
    delete_existing_folder, get_git_error, start_installation, validate_directory,
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
