//! File and directory deletion with retry logic and lock detection

use crate::config::ForceOpsConfig;
use crate::lock_checker::{self, LockCheckError, ProcessInfo};
use crate::process;
use crate::utils::{is_symlink, mark_as_not_readonly};
use anyhow::{Result, anyhow};
use std::fs;
use std::path::Path;
use std::thread;
use std::time::Duration;
use tracing::{info, warn};

// Use parallel remove_dir_all for fast directory deletion
use remove_dir_all::remove_dir_all as fast_remove_dir_all;

/// Handles deletion of files and directories with retry logic and process killing.
pub struct FileAndDirectoryDeleter {
    config: ForceOpsConfig,
}

impl FileAndDirectoryDeleter {
    pub fn new(config: ForceOpsConfig) -> Self {
        Self { config }
    }

    /// Delete a file or a folder, not following symlinks.
    /// If the delete fails, it will attempt to find processes using the file or directory.
    pub fn delete_file_or_directory(&self, path: &Path, force: bool) -> Result<()> {
        if path.is_file() {
            return self.delete_file(path);
        }

        if path.is_dir() {
            return self.delete_directory(path);
        }

        if !force {
            return Err(anyhow!(
                "Cannot remove '{}'. No such file or directory",
                path.display()
            ));
        }

        Ok(())
    }

    /// Delete a single file with retry logic.
    pub fn delete_file(&self, path: &Path) -> Result<()> {
        for attempt in 1..=self.config.max_retries + 1 {
            // Try to remove read-only attribute
            let _ = mark_as_not_readonly(path);

            match fs::remove_file(path) {
                Ok(()) => return Ok(()),
                Err(_e) if !path.exists() => return Ok(()), // File was deleted by something else
                Err(e) if is_io_or_permission_error(&e) => {
                    let get_processes = || -> Vec<ProcessInfo> {
                        match lock_checker::get_locking_processes(&[path]) {
                            Ok(procs) => procs,
                            Err(LockCheckError::GetList { code: 5, message }) => {
                                warn!(
                                    "Ignored exception: Failed to get entries (retry 0). (RmGetList() error 5: {})",
                                    message
                                );
                                Vec::new()
                            }
                            Err(_) => Vec::new(),
                        }
                    };

                    if self.kill_processes_and_log_info(false, attempt, path, get_processes) {
                        return Err(anyhow!("{}", e));
                    }
                }
                Err(e) => return Err(anyhow!("{}", e)),
            }
        }

        Err(anyhow!(
            "Failed to delete file '{}' after {} retries",
            path.display(),
            self.config.max_retries
        ))
    }

    /// Delete a directory recursively with retry logic.
    pub fn delete_directory(&self, path: &Path) -> Result<()> {
        if is_symlink(path) {
            // For symlinks, just remove the link itself
            return self.delete_empty_directory(path);
        }

        // Try fast parallel deletion first, with retry logic for locked directories
        for attempt in 1..=self.config.max_retries + 1 {
            match fast_remove_dir_all(path) {
                Ok(()) => return Ok(()),
                Err(_) if !path.exists() => return Ok(()),
                Err(e) => {
                    // Check if it's a sharing violation (locked file/directory)
                    let is_lock_error = e
                        .raw_os_error()
                        .is_some_and(|code| code == 32 || code == 33);

                    if is_lock_error {
                        let path_clone = path.to_path_buf();
                        let get_processes = || -> Vec<ProcessInfo> {
                            lock_checker::get_locking_processes_low_level(&path_clone)
                                .unwrap_or_default()
                        };

                        if self.kill_processes_and_log_info(true, attempt, path, get_processes) {
                            return Err(anyhow!("{}", e));
                        }
                        // Continue to next retry attempt
                    } else {
                        // Non-lock error, fall back to slow path for detailed errors
                        if path.exists() {
                            return self.delete_directory_slow(path);
                        }
                        return Err(anyhow!("{}", e));
                    }
                }
            }
        }

        Err(anyhow!(
            "Failed to delete directory '{}' after {} retries",
            path.display(),
            self.config.max_retries
        ))
    }

    /// Slow path: delete directory contents one by one with retry logic for each.
    fn delete_directory_slow(&self, path: &Path) -> Result<()> {
        self.delete_files_in_folder(path)?;
        self.delete_empty_directory(path)
    }

    /// Delete an empty directory with retry logic.
    fn delete_empty_directory(&self, path: &Path) -> Result<()> {
        for attempt in 1..=self.config.max_retries + 1 {
            // Try to remove read-only attribute
            let _ = mark_as_not_readonly(path);

            match fs::remove_dir(path) {
                Ok(()) => return Ok(()),
                Err(_e) if !path.exists() => return Ok(()), // Directory was deleted by something else
                Err(e) if is_io_error(&e) => {
                    let path_clone = path.to_path_buf();
                    let get_processes = || -> Vec<ProcessInfo> {
                        // For directories, use the low-level API (NtQuerySystemInformation)
                        lock_checker::get_locking_processes_low_level(&path_clone)
                            .unwrap_or_default()
                    };

                    if self.kill_processes_and_log_info(true, attempt, path, get_processes) {
                        return Err(anyhow!("{}", e));
                    }
                }
                Err(e) => return Err(anyhow!("{}", e)),
            }
        }

        Err(anyhow!(
            "Failed to delete directory '{}' after {} retries",
            path.display(),
            self.config.max_retries
        ))
    }

    fn delete_files_in_folder(&self, directory: &Path) -> Result<()> {
        let entries = fs::read_dir(directory)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                self.delete_file(&path)?;
            } else if path.is_dir() {
                self.delete_directory(&path)?;
            }
        }

        Ok(())
    }

    /// Kill processes and log information about the retry.
    /// Returns true if we should throw (exceeded retries), false otherwise.
    fn kill_processes_and_log_info<F>(
        &self,
        is_directory: bool,
        attempt_number: u32,
        path: &Path,
        get_processes: F,
    ) -> bool
    where
        F: FnOnce() -> Vec<ProcessInfo>,
    {
        let is_elevated = crate::elevation::is_process_elevated();
        let elevated_msg = if is_elevated {
            "ForceOps process is elevated"
        } else {
            "ForceOps process is not elevated"
        };

        if attempt_number > self.config.max_retries {
            info!(
                "Exceeded retry count of {}. Failed. {}.",
                self.config.max_retries, elevated_msg
            );
            return true;
        }

        let processes = get_processes();
        let file_or_dir = if is_directory { "directory" } else { "file" };
        let process_plural = if processes.len() == 1 {
            "process"
        } else {
            "processes"
        };

        let process_log_string: String = processes
            .iter()
            .map(|p| {
                format!(
                    "{} - {}",
                    p.process_id,
                    p.executable_name.as_deref().unwrap_or("")
                )
            })
            .collect::<Vec<_>>()
            .join(", ");

        info!(
            "Could not delete {} \"{}\". Beginning retry {}/{} in {}ms. {}. Found {} {} to try to kill: [{}].",
            file_or_dir,
            path.display(),
            attempt_number,
            self.config.max_retries,
            self.config.retry_delay_ms,
            elevated_msg,
            processes.len(),
            process_plural,
            process_log_string
        );

        thread::sleep(Duration::from_millis(self.config.retry_delay_ms));
        process::kill_processes(&processes);

        false
    }
}

fn is_io_or_permission_error(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::WouldBlock
    ) || is_io_error(error)
}

fn is_io_error(error: &std::io::Error) -> bool {
    // On Windows, "The process cannot access the file" is a sharing violation
    let raw_os_error = error.raw_os_error();
    matches!(raw_os_error, Some(32) | Some(33)) // ERROR_SHARING_VIOLATION | ERROR_LOCK_VIOLATION
        || matches!(error.kind(), std::io::ErrorKind::Other)
}
