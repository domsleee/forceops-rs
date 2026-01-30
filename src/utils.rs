use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// Combines a path with the current working directory and returns the absolute path.
pub fn combine_with_cwd_and_get_absolute_path(path: &str) -> PathBuf {
    let current_dir = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let combined = current_dir.join(path);
    fs::canonicalize(&combined).unwrap_or(combined)
}

/// Checks if a path is a symbolic link (reparse point on Windows).
pub fn is_symlink(path: &Path) -> bool {
    match fs::symlink_metadata(path) {
        Ok(metadata) => metadata.file_type().is_symlink(),
        Err(_) => false,
    }
}

/// Removes the read-only attribute from a file or directory if it's set.
pub fn mark_as_not_readonly(path: &Path) -> std::io::Result<()> {
    let metadata = fs::metadata(path)?;
    let mut permissions = metadata.permissions();

    if permissions.readonly() {
        permissions.set_readonly(false);
        fs::set_permissions(path, permissions)?;
    }

    Ok(())
}
