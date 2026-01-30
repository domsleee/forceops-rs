//! Tests for ListFileOrDirectoryLocks
//! Ported from ForceOps.Test/ListFileOrDirectoryLocksTest.cs

mod common;

use common::test_util::{
    create_temporary_directory, get_temporary_file_name, hold_lock_on_file_using_powershell,
    launch_process_in_directory,
};
use forceops::lock_checker;

#[test]
fn works_for_directory() {
    let temp_folder_path = get_temporary_file_name();
    let _temp_dir = create_temporary_directory(temp_folder_path.clone());
    let temp_path_str = temp_folder_path.to_string_lossy().to_string();

    let process = launch_process_in_directory(&temp_path_str);
    let pid = process.process.id();

    let locks = lock_checker::get_locks(&temp_folder_path);
    assert!(locks.is_ok(), "Should succeed getting locks");

    let locks = locks.unwrap();
    assert!(!locks.is_empty(), "Should find at least one lock");

    let found = locks.iter().any(|p| p.process_id == pid);
    assert!(found, "Should find our PowerShell process (pid: {})", pid);

    // Verify the executable name contains powershell
    let our_process = locks.iter().find(|p| p.process_id == pid).unwrap();
    let exe_name = our_process.executable_name.as_deref().unwrap_or("");
    assert!(
        exe_name.to_lowercase().contains("powershell"),
        "Executable name should contain 'powershell', got: {}",
        exe_name
    );
}

#[test]
fn works_for_file() {
    let temp_file_path = get_temporary_file_name();
    let temp_path_str = temp_file_path.to_string_lossy().to_string();

    let process = hold_lock_on_file_using_powershell(&temp_path_str);
    let pid = process.process.id();

    // The file should exist now (created by PowerShell)
    assert!(temp_file_path.exists(), "File should exist");

    let locks = lock_checker::get_locks(&temp_file_path);
    assert!(locks.is_ok(), "Should succeed getting locks: {:?}", locks);

    let locks = locks.unwrap();
    assert!(!locks.is_empty(), "Should find at least one lock");

    let found = locks.iter().any(|p| p.process_id == pid);
    assert!(found, "Should find our PowerShell process (pid: {})", pid);

    // Verify the executable name contains powershell
    let our_process = locks.iter().find(|p| p.process_id == pid).unwrap();
    let exe_name = our_process.executable_name.as_deref().unwrap_or("");
    assert!(
        exe_name.to_lowercase().contains("powershell"),
        "Executable name should contain 'powershell', got: {}",
        exe_name
    );
}

#[test]
fn file_not_found_error() {
    let non_existent_path = std::path::PathBuf::from(r"C:\C:\C:\");
    let result = lock_checker::get_locks(&non_existent_path);

    assert!(result.is_err(), "Should return error for non-existent path");
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("No such file or directory"),
        "Error should mention file not found: {}",
        err
    );
}
