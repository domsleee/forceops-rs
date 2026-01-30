//! Program/CLI tests
//! Ported from ForceOps.Test/ProgramTest.cs

mod common;

use common::test_util::{
    create_temporary_directory, get_temporary_file_name, launch_process_in_directory,
};
use std::fs::{self, File};
use std::path::PathBuf;
use std::process::Command;

fn get_forceops_exe() -> PathBuf {
    // Get the path to the built executable
    let mut path = std::env::current_exe().unwrap();
    path.pop(); // Remove test executable name
    path.pop(); // Remove deps
    path.push("fops.exe");
    path
}

#[test]
fn delete_multiple_files() {
    let temp_dir = get_temporary_file_name();
    fs::create_dir_all(&temp_dir).unwrap();

    let file1 = temp_dir.join("file1");
    let file2 = temp_dir.join("file2");
    File::create(&file1).unwrap();
    File::create(&file2).unwrap();

    assert!(file1.exists(), "file1 should exist");
    assert!(file2.exists(), "file2 should exist");

    let output = Command::new(get_forceops_exe())
        .args(["delete", &file1.to_string_lossy(), &file2.to_string_lossy()])
        .output()
        .expect("Failed to run forceops");

    assert!(output.status.success(), "forceops should succeed");
    assert!(!file1.exists(), "file1 should be deleted");
    assert!(!file2.exists(), "file2 should be deleted");

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn delete_non_existing_file_throws_message() {
    let output = Command::new(get_forceops_exe())
        .args(["delete", r"C:\C:\C:\"])
        .output()
        .expect("Failed to run forceops");

    assert!(
        !output.status.success(),
        "Should fail for non-existent file"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("No such file or directory"),
        "Should show file not found error: {}",
        stderr
    );
}

#[test]
fn delete_non_existing_file_with_force_succeeds() {
    let output = Command::new(get_forceops_exe())
        .args(["delete", "-f", r"C:\C:\C:\"])
        .output()
        .expect("Failed to run forceops");

    assert!(
        output.status.success(),
        "Should succeed with --force for non-existent file"
    );
}

#[test]
fn list_non_existing_file_throws_message() {
    let output = Command::new(get_forceops_exe())
        .args(["list", r"C:\C:\C:\"])
        .output()
        .expect("Failed to run forceops");

    assert!(
        !output.status.success(),
        "Should fail for non-existent file"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("No such file or directory"),
        "Should show file not found error: {}",
        stderr
    );
}

#[test]
fn retry_delay_and_max_retries_work() {
    let temp_dir = get_temporary_file_name();
    let _temp_dir_guard = create_temporary_directory(temp_dir.clone());
    let temp_path_str = temp_dir.to_string_lossy().to_string();

    let _process = launch_process_in_directory(&temp_path_str);

    let output = Command::new(get_forceops_exe())
        .args([
            "delete",
            &temp_path_str,
            "--retry-delay",
            "33",
            "--max-retries",
            "8",
            "--disable-elevate",
        ])
        .output()
        .expect("Failed to run forceops");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    // Check that the retry parameters are reflected in the output
    assert!(
        combined.contains("retry 1/8") || combined.contains("33ms"),
        "Output should reflect retry settings: {}",
        combined
    );
}

#[test]
fn help_command_works() {
    let output = Command::new(get_forceops_exe())
        .args(["--help"])
        .output()
        .expect("Failed to run forceops");

    assert!(output.status.success(), "Help should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("delete") && stdout.contains("list"),
        "Help should show commands"
    );
}

#[test]
fn rm_alias_works() {
    let temp_file = get_temporary_file_name();
    File::create(&temp_file).unwrap();
    assert!(temp_file.exists(), "File should exist");

    let output = Command::new(get_forceops_exe())
        .args(["rm", &temp_file.to_string_lossy()])
        .output()
        .expect("Failed to run forceops");

    assert!(output.status.success(), "rm alias should work");
    assert!(!temp_file.exists(), "File should be deleted");
}

#[test]
fn remove_alias_works() {
    let temp_file = get_temporary_file_name();
    File::create(&temp_file).unwrap();
    assert!(temp_file.exists(), "File should exist");

    let output = Command::new(get_forceops_exe())
        .args(["remove", &temp_file.to_string_lossy()])
        .output()
        .expect("Failed to run forceops");

    assert!(output.status.success(), "remove alias should work");
    assert!(!temp_file.exists(), "File should be deleted");
}

#[test]
fn list_command_output_format() {
    let temp_dir = get_temporary_file_name();
    let _temp_dir_guard = create_temporary_directory(temp_dir.clone());
    let temp_path_str = temp_dir.to_string_lossy().to_string();

    let process = launch_process_in_directory(&temp_path_str);
    let pid = process.process.id();

    let output = Command::new(get_forceops_exe())
        .args(["list", &temp_path_str])
        .output()
        .expect("Failed to run forceops");

    assert!(output.status.success(), "List should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Check header
    assert!(
        stdout.contains("ProcessId,ExecutableName,ApplicationName"),
        "Should have CSV header: {}",
        stdout
    );

    // Check our process is listed
    assert!(
        stdout.contains(&pid.to_string()),
        "Should list our process (pid: {}): {}",
        pid,
        stdout
    );

    // Check powershell is mentioned
    assert!(
        stdout.to_lowercase().contains("powershell"),
        "Should mention powershell: {}",
        stdout
    );
}
