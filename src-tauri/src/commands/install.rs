use crate::config::{REPO_URL, SLUS_FOLDER, SPARSE_PATH, TEMP_DIR_NAME};
use regex::Regex;
use serde::Serialize;
use std::io::{BufReader, Read as IoRead};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::fs;
use tauri::{Emitter, Window};

#[derive(Clone, Serialize)]
pub struct ProgressPayload {
    pub stage: String,
    pub message: String,
    pub percent: Option<u32>,
}

/// Get the path to git executable
/// On Windows, use bundled MinGit if available
/// On macOS, use system git
fn get_git_path() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        // Check for bundled MinGit first
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                // Check architecture
                let arch = if cfg!(target_arch = "aarch64") {
                    "arm64"
                } else {
                    "x64"
                };

                let mingit_path = exe_dir
                    .join("resources")
                    .join("mingit")
                    .join(arch)
                    .join("cmd")
                    .join("git.exe");

                if mingit_path.exists() {
                    return Ok(mingit_path.to_string_lossy().to_string());
                }
            }
        }

        // Fall back to system git
        if Command::new("git").arg("--version").output().is_ok() {
            return Ok("git".to_string());
        }

        Err("Git not found. Please ensure MinGit is bundled with the app or install Git.".to_string())
    }

    #[cfg(not(target_os = "windows"))]
    {
        // On macOS/Linux, check for system git
        if Command::new("git").arg("--version").output().is_ok() {
            return Ok("git".to_string());
        }

        Err("Git not found. Please install Xcode Command Line Tools by running: xcode-select --install".to_string())
    }
}

/// Check if git is available
#[tauri::command]
pub fn check_git_installed() -> Result<bool, String> {
    match get_git_path() {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

/// Get the git installation error message (for display to user)
#[tauri::command]
pub fn get_git_error() -> String {
    match get_git_path() {
        Ok(_) => String::new(),
        Err(e) => e,
    }
}

/// Detect the stage and percentage from git output
fn detect_git_stage(line: &str) -> (Option<&'static str>, Option<u32>) {
    let percent_re = Regex::new(r"(\d+)%").ok();
    let percent = percent_re
        .as_ref()
        .and_then(|re| re.captures(line))
        .and_then(|caps| caps.get(1))
        .and_then(|m| m.as_str().parse().ok());

    if line.contains("Receiving objects:") {
        return (Some("downloading"), percent);
    }
    if line.contains("Updating files:") {
        return (Some("extracting"), percent);
    }
    if line.contains("Resolving deltas:") {
        return (Some("downloading"), percent);
    }
    if line.contains("Compressing objects:") {
        return (Some("compressing"), percent);
    }
    if line.contains("Enumerating objects:") || line.contains("Counting objects:") {
        return (Some("compressing"), percent);
    }
    if line.contains("remote:") {
        return (Some("compressing"), percent);
    }

    (None, percent)
}

/// Read output handling both \r and \n as line terminators
/// Git uses \r to update progress on the same line
/// When detect_stages is false, always uses default_stage
fn read_output_with_progress<R: IoRead>(reader: R, window: &Window, default_stage: &str, detect_stages: bool) {
    let mut buf_reader = BufReader::new(reader);
    let mut buffer = Vec::new();
    let mut byte = [0u8; 1];

    loop {
        match buf_reader.read(&mut byte) {
            Ok(0) => break, // EOF
            Ok(_) => {
                if byte[0] == b'\r' || byte[0] == b'\n' {
                    if !buffer.is_empty() {
                        if let Ok(line) = String::from_utf8(buffer.clone()) {
                            let line = line.trim();
                            if !line.is_empty() {
                                let (detected_stage, percent) = detect_git_stage(line);
                                let stage = if detect_stages {
                                    detected_stage.unwrap_or(default_stage)
                                } else {
                                    default_stage
                                };

                                let _ = window.emit(
                                    "install-progress",
                                    ProgressPayload {
                                        stage: stage.to_string(),
                                        message: line.to_string(),
                                        percent,
                                    },
                                );
                            }
                        }
                        buffer.clear();
                    }
                } else {
                    buffer.push(byte[0]);
                }
            }
            Err(_) => break,
        }
    }

    // Handle any remaining data in buffer
    if !buffer.is_empty() {
        if let Ok(line) = String::from_utf8(buffer) {
            let line = line.trim();
            if !line.is_empty() {
                let (detected_stage, percent) = detect_git_stage(line);
                let stage = if detect_stages {
                    detected_stage.unwrap_or(default_stage)
                } else {
                    default_stage
                };

                let _ = window.emit(
                    "install-progress",
                    ProgressPayload {
                        stage: stage.to_string(),
                        message: line.to_string(),
                        percent,
                    },
                );
            }
        }
    }
}

/// Run a git command with PTY support (using script command on macOS/Linux)
/// This ensures git outputs progress even when not connected to a real terminal
/// Uses caffeinate to prevent system sleep during long operations
/// When detect_stages is false, always uses default_stage instead of detecting from output
#[cfg(not(target_os = "windows"))]
fn run_git_with_pty(
    git_path: &str,
    args: &[&str],
    working_dir: &PathBuf,
    window: &Window,
    default_stage: &str,
    detect_stages: bool,
) -> Result<bool, String> {
    // Use 'caffeinate' to prevent sleep, 'script' to create a PTY for git
    // caffeinate -d: prevent display sleep (also prevents screensaver)
    // script -q /dev/null: create PTY without saving typescript
    let mut cmd_args: Vec<&str> = vec!["-d", "script", "-q", "/dev/null", git_path];
    cmd_args.extend(args);

    let mut cmd = Command::new("caffeinate")
        .args(&cmd_args)
        .current_dir(working_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start command: {}", e))?;

    // script command outputs everything to stdout (including what would normally be stderr)
    if let Some(stdout) = cmd.stdout.take() {
        read_output_with_progress(stdout, window, default_stage, detect_stages);
    }

    let status = cmd
        .wait()
        .map_err(|e| format!("Command failed: {}", e))?;

    Ok(status.success())
}

/// Run a git command on Windows using ConPTY for proper progress output
/// Uses SetThreadExecutionState to prevent system sleep during long operations
/// When detect_stages is false, always uses default_stage instead of detecting from output
#[cfg(target_os = "windows")]
fn run_git_with_pty(
    git_path: &str,
    args: &[&str],
    working_dir: &PathBuf,
    window: &Window,
    default_stage: &str,
    detect_stages: bool,
) -> Result<bool, String> {
    use conpty::spawn;
    use std::io::Read as _;
    use windows::Win32::System::Power::{SetThreadExecutionState, ES_CONTINUOUS, ES_SYSTEM_REQUIRED, ES_DISPLAY_REQUIRED};

    // Prevent system sleep during the operation
    unsafe {
        SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED);
    }

    // Build full command line
    let git_args: Vec<String> = args.iter().map(|s| {
        if s.contains(' ') {
            format!("\"{}\"", s)
        } else {
            s.to_string()
        }
    }).collect();

    let command_line = format!("{} {}", git_path, git_args.join(" "));

    // Spawn process using ConPTY (Windows Pseudo Console)
    // This makes git think it's connected to a real terminal
    let mut proc = spawn(&command_line)
        .map_err(|e| {
            unsafe { SetThreadExecutionState(ES_CONTINUOUS); }
            format!("Failed to spawn process with ConPTY: {}", e)
        })?;

    // Set working directory isn't directly supported by conpty::spawn,
    // so we need to cd first
    // Actually, let's use a different approach - build a cmd command that cd's first
    drop(proc);

    let cd_and_run = format!("cd /d \"{}\" && {}", working_dir.display(), command_line);
    let mut proc = spawn(&format!("cmd /c {}", cd_and_run))
        .map_err(|e| {
            unsafe { SetThreadExecutionState(ES_CONTINUOUS); }
            format!("Failed to spawn process with ConPTY: {}", e)
        })?;

    // Read output from the PTY
    let mut output = proc.output().map_err(|e| {
        unsafe { SetThreadExecutionState(ES_CONTINUOUS); }
        format!("Failed to get process output: {}", e)
    })?;

    let mut buffer = [0u8; 1];
    let mut line_buffer = Vec::new();

    loop {
        match output.read(&mut buffer) {
            Ok(0) => break, // EOF
            Ok(_) => {
                let byte = buffer[0];
                if byte == b'\r' || byte == b'\n' {
                    if !line_buffer.is_empty() {
                        if let Ok(line) = String::from_utf8(line_buffer.clone()) {
                            let line = line.trim();
                            if !line.is_empty() {
                                let (detected_stage, percent) = detect_git_stage(line);
                                let stage = if detect_stages {
                                    detected_stage.unwrap_or(default_stage)
                                } else {
                                    default_stage
                                };

                                let _ = window.emit(
                                    "install-progress",
                                    ProgressPayload {
                                        stage: stage.to_string(),
                                        message: line.to_string(),
                                        percent,
                                    },
                                );
                            }
                        }
                        line_buffer.clear();
                    }
                } else {
                    line_buffer.push(byte);
                }
            }
            Err(_) => break,
        }
    }

    // Handle remaining data
    if !line_buffer.is_empty() {
        if let Ok(line) = String::from_utf8(line_buffer) {
            let line = line.trim();
            if !line.is_empty() {
                let (detected_stage, percent) = detect_git_stage(line);
                let stage = if detect_stages {
                    detected_stage.unwrap_or(default_stage)
                } else {
                    default_stage
                };

                let _ = window.emit(
                    "install-progress",
                    ProgressPayload {
                        stage: stage.to_string(),
                        message: line.to_string(),
                        percent,
                    },
                );
            }
        }
    }

    // Wait for process to exit and check status
    let success = proc.wait(None).map_err(|e| {
        unsafe { SetThreadExecutionState(ES_CONTINUOUS); }
        format!("Failed to wait for process: {}", e)
    })?;

    // Restore normal sleep behavior
    unsafe {
        SetThreadExecutionState(ES_CONTINUOUS);
    }

    Ok(success)
}

/// Run the git sparse checkout installation
#[tauri::command]
pub async fn start_installation(textures_dir: String, window: Window) -> Result<(), String> {
    let git_path = get_git_path()?;
    let textures_path = PathBuf::from(&textures_dir);
    let temp_path = textures_path.join(TEMP_DIR_NAME);
    let final_path = textures_path.join(SLUS_FOLDER);

    // Emit initial progress
    let _ = window.emit(
        "install-progress",
        ProgressPayload {
            stage: "preparing".to_string(),
            message: "Preparing installation...".to_string(),
            percent: Some(0),
        },
    );

    // Clean up any existing temp directory
    if temp_path.exists() {
        fs::remove_dir_all(&temp_path)
            .map_err(|e| format!("Failed to clean temp directory: {}", e))?;
    }

    // Create temp directory
    fs::create_dir_all(&temp_path)
        .map_err(|e| format!("Failed to create temp directory: {}", e))?;

    // Stage 1: Clone with sparse checkout (this is quick - just metadata)
    let _ = window.emit(
        "install-progress",
        ProgressPayload {
            stage: "cloning".to_string(),
            message: "Initializing repository...".to_string(),
            percent: Some(0),
        },
    );

    let clone_success = run_git_with_pty(
        &git_path,
        &[
            "clone",
            "--depth=1",
            "--filter=blob:none",
            "--sparse",
            "--progress",
            REPO_URL,
            ".",
        ],
        &temp_path,
        &window,
        "cloning",
        false, // Don't detect stages - keep showing "Initializing repository..."
    )?;

    if !clone_success {
        let _ = fs::remove_dir_all(&temp_path);
        return Err("Git clone failed. Please check your internet connection.".to_string());
    }

    // Stage 2: Set sparse checkout path - THIS IS THE MAIN DOWNLOAD
    let _ = window.emit(
        "install-progress",
        ProgressPayload {
            stage: "downloading".to_string(),
            message: format!("Starting download of {}...", SPARSE_PATH),
            percent: Some(0),
        },
    );

    let checkout_success = run_git_with_pty(
        &git_path,
        &["sparse-checkout", "set", SPARSE_PATH],
        &temp_path,
        &window,
        "downloading",
        true, // Detect stages - show compressing/downloading/extracting
    )?;

    if !checkout_success {
        let _ = fs::remove_dir_all(&temp_path);
        return Err("Sparse checkout failed.".to_string());
    }

    // Stage 3: Move folder to final location
    let _ = window.emit(
        "install-progress",
        ProgressPayload {
            stage: "moving".to_string(),
            message: format!("Moving {} to final location...", SLUS_FOLDER),
            percent: Some(0),
        },
    );

    let source_path = temp_path.join("textures").join(SLUS_FOLDER);

    if !source_path.exists() {
        let _ = fs::remove_dir_all(&temp_path);
        return Err(format!(
            "Expected folder {} not found in repository",
            SPARSE_PATH
        ));
    }

    // Move the folder
    fs::rename(&source_path, &final_path)
        .map_err(|e| format!("Failed to move folder to final location: {}", e))?;

    // Stage 4: Cleanup
    let _ = window.emit(
        "install-progress",
        ProgressPayload {
            stage: "cleanup".to_string(),
            message: "Cleaning up temporary files...".to_string(),
            percent: Some(0),
        },
    );

    fs::remove_dir_all(&temp_path)
        .map_err(|e| format!("Failed to clean up temp directory: {}", e))?;

    // Done!
    let _ = window.emit(
        "install-progress",
        ProgressPayload {
            stage: "complete".to_string(),
            message: format!(
                "Installation complete! Textures installed to: {}",
                final_path.display()
            ),
            percent: Some(100),
        },
    );

    Ok(())
}
