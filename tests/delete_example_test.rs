//! Integration test - end to end test using the binary
//! Ported from ForceOps.Test/DeleteExampleTest.cs

mod common;

use common::test_util::{get_temporary_file_name, hold_lock_on_file_using_powershell};
use std::path::PathBuf;
use std::process::Command;

fn get_forceops_exe() -> PathBuf {
    let mut path = std::env::current_exe().unwrap();
    path.pop(); // Remove test executable name
    path.pop(); // Remove deps
    path.push("fops.exe");
    path
}

#[test]
fn delete_example_works() {
    let temp_file_path = get_temporary_file_name();
    let temp_path_str = temp_file_path.to_string_lossy().to_string();

    // Create a process holding a lock on the file
    let _process = hold_lock_on_file_using_powershell(&temp_path_str);

    // Verify file exists
    assert!(temp_file_path.exists(), "File should exist");

    // Run forceops to delete the locked file
    let output = Command::new(get_forceops_exe())
        .args(["delete", "--disable-elevate", &temp_path_str])
        .output()
        .expect("Failed to run forceops");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    assert!(
        output.status.success(),
        "forceops should succeed. Output: {}",
        combined
    );
    assert!(!temp_file_path.exists(), "File should be deleted");

    // Should have logged about the locked file
    assert!(
        combined.contains("Could not delete file") || combined.contains("retry"),
        "Should log about retry attempt: {}",
        combined
    );
}

#[test]
fn delete_unlocked_file_works() {
    let temp_file_path = get_temporary_file_name();

    // Create the file (not locked)
    std::fs::File::create(&temp_file_path).expect("Failed to create temp file");
    assert!(temp_file_path.exists(), "File should exist");

    // Run forceops to delete the file
    let output = Command::new(get_forceops_exe())
        .args(["delete", &temp_file_path.to_string_lossy()])
        .output()
        .expect("Failed to run forceops");

    assert!(output.status.success(), "forceops should succeed");
    assert!(!temp_file_path.exists(), "File should be deleted");
}

#[test]
fn delete_directory_recursively() {
    let temp_dir = get_temporary_file_name();
    std::fs::create_dir_all(&temp_dir).expect("Failed to create temp directory");

    // Create some files in the directory
    let file1 = temp_dir.join("file1.txt");
    let file2 = temp_dir.join("file2.txt");
    let subdir = temp_dir.join("subdir");
    std::fs::create_dir_all(&subdir).expect("Failed to create subdir");
    let file3 = subdir.join("file3.txt");

    std::fs::File::create(&file1).unwrap();
    std::fs::File::create(&file2).unwrap();
    std::fs::File::create(&file3).unwrap();

    assert!(temp_dir.exists(), "Directory should exist");

    // Run forceops to delete the directory
    let output = Command::new(get_forceops_exe())
        .args(["delete", &temp_dir.to_string_lossy()])
        .output()
        .expect("Failed to run forceops");

    assert!(
        output.status.success(),
        "forceops should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!temp_dir.exists(), "Directory should be deleted");
}
