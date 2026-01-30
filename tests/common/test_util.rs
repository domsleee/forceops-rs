//! Test utilities for forceops tests

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use super::wrapped_process::WrappedProcess;

/// Launch a PowerShell process with its working directory set to the specified path
pub fn launch_process_in_directory(working_directory: &str) -> WrappedProcess {
    launch_powershell_with_command("", working_directory)
}

/// Launch a PowerShell process that holds a lock on the specified file
pub fn hold_lock_on_file_using_powershell(file_path: &str) -> WrappedProcess {
    let command = format!(
        "$file = [System.IO.File]::Open('{}', 'CreateNew')",
        file_path
    );
    launch_powershell_with_command(&command, "")
}

fn launch_powershell_with_command(command: &str, working_directory: &str) -> WrappedProcess {
    let full_command = format!(
        "$ErrorActionPreference='stop'; {}; echo 'process has been loaded'; sleep 10000",
        command
    );

    let mut cmd = Command::new("powershell");
    cmd.args(["-NoProfile", "-Command", &full_command])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if !working_directory.is_empty() {
        cmd.current_dir(working_directory);
    }

    let mut process = cmd.spawn().expect("Failed to start PowerShell process");

    // Wait for the process to be ready
    wait_for_process_loaded_message(&mut process);

    WrappedProcess::new(process)
}

fn wait_for_process_loaded_message(process: &mut std::process::Child) {
    let stdout = process.stdout.take().expect("Failed to get stdout");
    let reader = BufReader::new(stdout);

    let start_time = Instant::now();
    let timeout = Duration::from_secs(5);

    for line in reader.lines() {
        if start_time.elapsed() > timeout {
            panic!("Timeout waiting for process to load");
        }

        match line {
            Ok(text) if text.contains("process has been loaded") => {
                return;
            }
            Ok(_) => continue,
            Err(e) => panic!("Error reading stdout: {}", e),
        }
    }

    panic!("Process exited before becoming ready");
}

/// Generate a unique temporary file path
pub fn get_temporary_file_name() -> PathBuf {
    let temp_dir = std::env::temp_dir();
    temp_dir.join(uuid::Uuid::new_v4().to_string())
}

/// Create a temporary directory that gets deleted when the guard is dropped
pub struct TempDirectory {
    path: PathBuf,
}

impl TempDirectory {
    pub fn new(path: PathBuf) -> Self {
        fs::create_dir_all(&path).expect("Failed to create temp directory");
        Self { path }
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }
}

impl Drop for TempDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// Create a temporary directory at the given path
pub fn create_temporary_directory(path: PathBuf) -> TempDirectory {
    TempDirectory::new(path)
}
