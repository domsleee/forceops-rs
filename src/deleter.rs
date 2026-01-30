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
            // For symlinks, just remove the symlink itself (not its contents)
            let _ = mark_as_not_readonly(path);
            return fs::remove_dir(path).map_err(|e| anyhow!("{}", e));
        }

        // Try fast parallel deletion first
        if fast_remove_dir_all(path).is_ok() {
            return Ok(());
        }

        // Fast path failed - check if directory still exists
        if !path.exists() {
            return Ok(());
        }

        // Fall back to slow path with per-file retry logic
        // This handles locked files/directories properly
        self.delete_directory_with_retry(path)
    }

    /// Delete directory with full retry logic including process killing.
    fn delete_directory_with_retry(&self, path: &Path) -> Result<()> {
        for attempt in 1..=self.config.max_retries + 1 {
            // Delete contents first (if not a symlink)
            if !is_symlink(path)
                && let Err(e) = self.delete_files_in_folder_once(path)
            {
                // If deleting contents fails, try to kill processes and retry
                if attempt <= self.config.max_retries {
                    let path_clone = path.to_path_buf();
                    let get_processes = || -> Vec<ProcessInfo> {
                        lock_checker::get_locking_processes_low_level(&path_clone)
                            .unwrap_or_default()
                    };
                    self.kill_processes_and_log_info(true, attempt, path, get_processes);
                    continue;
                }
                return Err(e);
            }

            // Try to remove the directory itself
            let _ = mark_as_not_readonly(path);
            match fs::remove_dir(path) {
                Ok(()) => return Ok(()),
                Err(_) if !path.exists() => return Ok(()),
                Err(e) if is_io_error(&e) => {
                    let path_clone = path.to_path_buf();
                    let get_processes = || -> Vec<ProcessInfo> {
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

    /// Try to delete files in folder once, without retrying individual files.
    fn delete_files_in_folder_once(&self, directory: &Path) -> Result<()> {
        let entries = match fs::read_dir(directory) {
            Ok(e) => e,
            Err(_) if !directory.exists() => return Ok(()),
            Err(e) => return Err(anyhow!("{}", e)),
        };

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                // Try fast delete, ignore errors (will retry at directory level)
                let _ = mark_as_not_readonly(&path);
                if let Err(e) = fs::remove_file(&path)
                    && path.exists()
                    && is_io_or_permission_error(&e)
                {
                    return Err(anyhow!("{}", e));
                }
            } else if path.is_dir() {
                // Recursively try to delete subdirectory
                self.delete_directory_with_retry(&path)?;
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
