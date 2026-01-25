use crate::config::{REPO_URL, SLUS_FOLDER, SPARSE_PATH, TEMP_DIR_NAME};
use regex::Regex;
use serde::Serialize;
use std::io::{BufReader, Read as IoRead};
use std::path::PathBuf;
#[cfg(not(target_os = "windows"))]
use std::process::{Command, Stdio};
#[cfg(target_os = "windows")]
use std::process::Command;
use std::fs;
use std::sync::Mutex;
use tauri::{Emitter, Window};

// Track running process PIDs so we can kill them on app exit
static RUNNING_PIDS: Mutex<Vec<u32>> = Mutex::new(Vec::new());

/// Kill all tracked processes (called on app exit)
pub fn cleanup_processes() {
    if let Ok(pids) = RUNNING_PIDS.lock() {
        for pid in pids.iter() {
            #[cfg(target_os = "windows")]
            {
                // Use taskkill to kill the process tree
                let _ = Command::new("taskkill")
                    .args(["/F", "/T", "/PID", &pid.to_string()])
                    .output();
            }
            #[cfg(not(target_os = "windows"))]
            {
                // On Unix, kill the process group
                let _ = Command::new("kill")
                    .args(["-9", &pid.to_string()])
                    .output();
            }
        }
    }
}

#[derive(Clone, Serialize)]
pub struct ProgressPayload {
    pub stage: String,
    pub message: String,
    pub percent: Option<u32>,
}

/// Get the path to git executable
/// On Windows x64, use bundled MinGit if available
/// On Windows ARM, require system git
/// On macOS, use system git
fn get_git_path() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        let is_arm = cfg!(target_arch = "aarch64");

        // On x64, check for bundled MinGit first
        if !is_arm {
            if let Ok(exe_path) = std::env::current_exe() {
                if let Some(exe_dir) = exe_path.parent() {
                    // MinGit is bundled at resources/mingit/x64/
                    let mingit_path = exe_dir
                        .join("resources")
                        .join("mingit")
                        .join("x64")
                        .join("cmd")
                        .join("git.exe");

                    if mingit_path.exists() {
                        return Ok(mingit_path.to_string_lossy().to_string());
                    }
                }
            }
        }

        // Fall back to system git
        if Command::new("git").arg("--version").output().is_ok() {
            return Ok("git".to_string());
        }

        // Build error message based on architecture
        if is_arm {
            Err("Git not found. On Windows ARM, please install Git manually from https://git-scm.com/download/win".to_string())
        } else {
            let mut err_msg = String::from("Git not found. Searched locations:\n");
            if let Ok(exe_path) = std::env::current_exe() {
                if let Some(exe_dir) = exe_path.parent() {
                    err_msg.push_str(&format!("  - {}\\resources\\mingit\\x64\\cmd\\git.exe\n", exe_dir.display()));
                }
            }
            err_msg.push_str("  - System PATH\n");
            err_msg.push_str("\nPlease reinstall the app or install Git from https://git-scm.com/download/win");
            Err(err_msg)
        }
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

/// Strip ANSI escape codes from a string
fn strip_ansi_codes(s: &str) -> String {
    let ansi_re = Regex::new(r"\x1b\[[0-9;]*[a-zA-Z]").unwrap();
    ansi_re.replace_all(s, "").to_string()
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
                            let line = strip_ansi_codes(line.trim());
                            if !line.is_empty() {
                                let (detected_stage, percent) = detect_git_stage(&line);
                                let stage = if detect_stages {
                                    detected_stage.unwrap_or(default_stage)
                                } else {
                                    default_stage
                                };

                                let _ = window.emit(
                                    "install-progress",
                                    ProgressPayload {
                                        stage: stage.to_string(),
                                        message: line,
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
            let line = strip_ansi_codes(line.trim());
            if !line.is_empty() {
                let (detected_stage, percent) = detect_git_stage(&line);
                let stage = if detect_stages {
                    detected_stage.unwrap_or(default_stage)
                } else {
                    default_stage
                };

                let _ = window.emit(
                    "install-progress",
                    ProgressPayload {
                        stage: stage.to_string(),
                        message: line,
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

    let working_dir_str = working_dir.to_string_lossy().to_string();

    // Build command arguments
    // For clone command, replace "." destination with full path
    // For other commands, use -C flag to set working directory
    let is_clone = args.first() == Some(&"clone");

    let full_args: Vec<String> = if is_clone {
        // For clone, replace "." with the full path
        args.iter().map(|arg| {
            if *arg == "." {
                format!("\"{}\"", working_dir_str)
            } else if arg.contains(' ') {
                format!("\"{}\"", arg)
            } else {
                arg.to_string()
            }
        }).collect()
    } else {
        // For other commands, use -C flag
        let mut v: Vec<String> = vec![
            "-C".to_string(),
            format!("\"{}\"", working_dir_str),
        ];
        for arg in args {
            if arg.contains(' ') {
                v.push(format!("\"{}\"", arg));
            } else {
                v.push(arg.to_string());
            }
        }
        v
    };

    let command_line = format!("{} {}", git_path, full_args.join(" "));

    // Spawn process using ConPTY (Windows Pseudo Console)
    // This makes git think it's connected to a real terminal
    let mut proc = spawn(&command_line)
        .map_err(|e| {
            unsafe { SetThreadExecutionState(ES_CONTINUOUS); }
            format!("Failed to spawn process with ConPTY: {}", e)
        })?;

    // Track the PID so we can kill it if the app closes
    let pid = proc.pid();
    if let Ok(mut pids) = RUNNING_PIDS.lock() {
        pids.push(pid);
    }

    // Read output from the PTY in a separate thread
    // This prevents blocking if the PTY doesn't send EOF properly
    let output = proc.output().map_err(|e| {
        unsafe { SetThreadExecutionState(ES_CONTINUOUS); }
        format!("Failed to get process output: {}", e)
    })?;

    let window_clone = window.clone();
    let default_stage_owned = default_stage.to_string();

    let reader_handle = std::thread::spawn(move || {
        let mut output = output;
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
                                        detected_stage.unwrap_or(&default_stage_owned)
                                    } else {
                                        &default_stage_owned
                                    };

                                    let _ = window_clone.emit(
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
    });

    // Wait for process to exit (this returns even if reader is still running)
    let exit_code = proc.wait(None).map_err(|e| {
        unsafe { SetThreadExecutionState(ES_CONTINUOUS); }
        format!("Failed to wait for process: {}", e)
    })?;

    // Remove PID from tracking list
    if let Ok(mut pids) = RUNNING_PIDS.lock() {
        pids.retain(|&p| p != pid);
    }

    // Drop proc to close the PTY, which should cause the reader to get EOF
    drop(proc);

    // Give the reader thread a short time to finish reading any buffered output
    // Don't block forever - if it's stuck, just move on
    let join_timeout = std::time::Duration::from_secs(2);
    let start = std::time::Instant::now();
    while !reader_handle.is_finished() && start.elapsed() < join_timeout {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    // Don't call join() - if thread is stuck, let it be orphaned

    // Restore normal sleep behavior
    unsafe {
        SetThreadExecutionState(ES_CONTINUOUS);
    }

    // Exit code 0 means success
    Ok(exit_code == 0)
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

    // Create temp directory (only on macOS - on Windows, git clone will create it)
    #[cfg(not(target_os = "windows"))]
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
