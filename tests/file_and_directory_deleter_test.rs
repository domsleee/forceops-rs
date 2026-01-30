//! Tests for FileAndDirectoryDeleter
//! Ported from ForceOps.Test/FileAndDirectoryDeleterTest.cs

mod common;

use common::test_util::{
    create_temporary_directory, get_temporary_file_name, hold_lock_on_file_using_powershell,
    launch_process_in_directory,
};
use fops::config::ForceOpsConfig;
use fops::deleter::FileAndDirectoryDeleter;
use std::fs;

#[test]
fn deleting_directory_open_in_powershell_working_directory() {
    let temp_folder_path = get_temporary_file_name();
    let _temp_dir = create_temporary_directory(temp_folder_path.clone());
    let temp_path_str = temp_folder_path.to_string_lossy().to_string();

    let _process = launch_process_in_directory(&temp_path_str);

    // With no retries, should fail
    let config_no_retries = ForceOpsConfig {
        max_retries: 0,
        retry_delay_ms: 50,
        disable_elevate: true,
    };
    let deleter = FileAndDirectoryDeleter::new(config_no_retries);
    let result = deleter.delete_directory(&temp_folder_path);
    assert!(result.is_err(), "Should fail with no retries");

    // With retries, should succeed after killing the process
    let config_with_retries = ForceOpsConfig {
        max_retries: 10,
        retry_delay_ms: 50,
        disable_elevate: true,
    };
    let deleter = FileAndDirectoryDeleter::new(config_with_retries);
    let result = deleter.delete_directory(&temp_folder_path);
    assert!(result.is_ok(), "Should succeed with retries: {:?}", result);
    assert!(!temp_folder_path.exists(), "Directory should be deleted");
}

#[test]
fn deleting_readonly_directory_open_in_powershell_working_directory() {
    let temp_folder_path = get_temporary_file_name();
    let _temp_dir = create_temporary_directory(temp_folder_path.clone());
    let temp_path_str = temp_folder_path.to_string_lossy().to_string();

    let _process = launch_process_in_directory(&temp_path_str);

    // Make directory read-only
    let mut perms = fs::metadata(&temp_folder_path).unwrap().permissions();
    perms.set_readonly(true);
    fs::set_permissions(&temp_folder_path, perms).unwrap();

    // With no retries, should fail
    let config_no_retries = ForceOpsConfig {
        max_retries: 0,
        retry_delay_ms: 50,
        disable_elevate: true,
    };
    let deleter = FileAndDirectoryDeleter::new(config_no_retries);
    let result = deleter.delete_directory(&temp_folder_path);
    assert!(result.is_err(), "Should fail with no retries");

    // With retries, should succeed after killing the process
    let config_with_retries = ForceOpsConfig {
        max_retries: 10,
        retry_delay_ms: 50,
        disable_elevate: true,
    };
    let deleter = FileAndDirectoryDeleter::new(config_with_retries);
    let result = deleter.delete_directory(&temp_folder_path);
    assert!(result.is_ok(), "Should succeed with retries: {:?}", result);
    assert!(!temp_folder_path.exists(), "Directory should be deleted");
}

#[test]
fn deleting_file_open_by_powershell() {
    let temp_file_path = get_temporary_file_name();
    let temp_path_str = temp_file_path.to_string_lossy().to_string();

    let _process = hold_lock_on_file_using_powershell(&temp_path_str);

    // With no retries, should fail
    let config_no_retries = ForceOpsConfig {
        max_retries: 0,
        retry_delay_ms: 50,
        disable_elevate: true,
    };
    let deleter = FileAndDirectoryDeleter::new(config_no_retries);
    let result = deleter.delete_file(&temp_file_path);
    assert!(result.is_err(), "Should fail with no retries");

    // With retries, should succeed after killing the process
    let config_with_retries = ForceOpsConfig {
        max_retries: 10,
        retry_delay_ms: 50,
        disable_elevate: true,
    };
    let deleter = FileAndDirectoryDeleter::new(config_with_retries);
    let result = deleter.delete_file(&temp_file_path);
    assert!(result.is_ok(), "Should succeed with retries: {:?}", result);
    assert!(!temp_file_path.exists(), "File should be deleted");
}

#[test]
fn deleting_readonly_file_open_by_powershell() {
    let temp_file_path = get_temporary_file_name();
    let temp_path_str = temp_file_path.to_string_lossy().to_string();

    let _process = hold_lock_on_file_using_powershell(&temp_path_str);

    // Make file read-only
    let mut perms = fs::metadata(&temp_file_path).unwrap().permissions();
    perms.set_readonly(true);
    fs::set_permissions(&temp_file_path, perms).unwrap();

    // With no retries, should fail
    let config_no_retries = ForceOpsConfig {
        max_retries: 0,
        retry_delay_ms: 50,
        disable_elevate: true,
    };
    let deleter = FileAndDirectoryDeleter::new(config_no_retries);
    let result = deleter.delete_file(&temp_file_path);
    assert!(result.is_err(), "Should fail with no retries");

    // With retries, should succeed after killing the process
    let config_with_retries = ForceOpsConfig {
        max_retries: 10,
        retry_delay_ms: 50,
        disable_elevate: true,
    };
    let deleter = FileAndDirectoryDeleter::new(config_with_retries);
    let result = deleter.delete_file(&temp_file_path);
    assert!(result.is_ok(), "Should succeed with retries: {:?}", result);
    assert!(!temp_file_path.exists(), "File should be deleted");
}
