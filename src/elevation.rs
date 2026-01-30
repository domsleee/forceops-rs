//! Elevation utilities for Windows
//!
//! Provides functionality to check if the current process is elevated (running as admin)
//! and to relaunch the process with elevated privileges.

use anyhow::{Result, anyhow};
use std::ffi::OsStr;
use std::io::{BufRead, BufReader};
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::ptr;
use tracing::info;
use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows::Win32::Security::{GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation};
use windows::Win32::System::Threading::{
    GetCurrentProcess, INFINITE, OpenProcessToken, WaitForSingleObject,
};
use windows::Win32::UI::Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW};
use windows::core::PCWSTR;

/// Checks if the current process is running with elevated (administrator) privileges.
pub fn is_process_elevated() -> bool {
    unsafe {
        let mut token_handle: HANDLE = HANDLE::default();

        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token_handle).is_err() {
            return false;
        }

        let mut elevation = TOKEN_ELEVATION::default();
        let mut return_length: u32 = 0;

        let result = GetTokenInformation(
            token_handle,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut _),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut return_length,
        );

        let _ = CloseHandle(token_handle);

        result.is_ok() && elevation.TokenIsElevated != 0
    }
}

/// Runs an action and relaunches as elevated if it fails with a permission error.
pub fn run_with_relaunch_as_elevated<F, A>(action: F, build_args: A) -> Result<()>
where
    F: FnOnce() -> Result<()>,
    A: FnOnce() -> Vec<String>,
{
    match action() {
        Ok(()) => Ok(()),
        Err(e) if is_permission_error(&e) && !is_process_elevated() => {
            let args = build_args();
            let output_file =
                std::env::temp_dir().join(format!("forceops_{}.tmp", std::process::id()));

            info!(
                "Unable to perform operation as an unelevated process. Retrying as elevated and logging to \"{}\".",
                output_file.display()
            );

            let exit_code = relaunch_as_elevated(&args, &output_file)?;

            if exit_code != 0 {
                // Read and display the output from the elevated process
                if let Ok(file) = std::fs::File::open(&output_file) {
                    let reader = BufReader::new(file);
                    for line in reader.lines().map_while(Result::ok) {
                        eprintln!("{}", line);
                    }
                }
                let _ = std::fs::remove_file(&output_file);
                Err(anyhow!("Child process failed with exit code {}", exit_code))
            } else {
                info!("Successfully deleted as admin");
                let _ = std::fs::remove_file(&output_file);
                Ok(())
            }
        }
        Err(e) => Err(e),
    }
}

fn is_permission_error(error: &anyhow::Error) -> bool {
    let err_string = error.to_string().to_lowercase();
    err_string.contains("access")
        || err_string.contains("permission")
        || err_string.contains("denied")
}

/// Relaunches the current executable with elevated privileges.
fn relaunch_as_elevated(args: &[String], output_file: &Path) -> Result<u32> {
    let exe_path = std::env::current_exe()?;

    // Build command line: skip first arg (exe name), add output redirection
    let args_str = args
        .iter()
        .skip(1)
        .map(|s| {
            if s.contains(' ') {
                format!("\"{}\"", s)
            } else {
                s.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ");

    // Use cmd.exe to handle redirection
    let cmd_args = format!(
        "/c \"\"{}\" {} 2>&1 > \"{}\"\"",
        exe_path.display(),
        args_str,
        output_file.display()
    );

    let verb: Vec<u16> = OsStr::new("runas").encode_wide().chain(Some(0)).collect();
    let file: Vec<u16> = OsStr::new("cmd.exe").encode_wide().chain(Some(0)).collect();
    let params: Vec<u16> = OsStr::new(&cmd_args).encode_wide().chain(Some(0)).collect();
    let dir: Vec<u16> = std::env::current_dir()
        .unwrap_or_default()
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();

    unsafe {
        let mut sei = SHELLEXECUTEINFOW {
            cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_NOCLOSEPROCESS,
            hwnd: windows::Win32::Foundation::HWND::default(),
            lpVerb: PCWSTR(verb.as_ptr()),
            lpFile: PCWSTR(file.as_ptr()),
            lpParameters: PCWSTR(params.as_ptr()),
            lpDirectory: PCWSTR(dir.as_ptr()),
            nShow: 0, // SW_HIDE
            hInstApp: windows::Win32::Foundation::HINSTANCE::default(),
            lpIDList: ptr::null_mut(),
            lpClass: PCWSTR::null(),
            hkeyClass: windows::Win32::System::Registry::HKEY::default(),
            dwHotKey: 0,
            Anonymous: Default::default(),
            hProcess: HANDLE::default(),
        };

        if ShellExecuteExW(&mut sei).is_err() {
            return Err(anyhow!("Failed to launch elevated process"));
        }

        if sei.hProcess.is_invalid() {
            return Err(anyhow!("Failed to get process handle"));
        }

        // Wait for the process to complete
        let wait_result = WaitForSingleObject(sei.hProcess, INFINITE);

        if wait_result != WAIT_OBJECT_0 {
            let _ = CloseHandle(sei.hProcess);
            return Err(anyhow!("Failed to wait for elevated process"));
        }

        // Get exit code
        let mut exit_code: u32 = 0;
        windows::Win32::System::Threading::GetExitCodeProcess(sei.hProcess, &mut exit_code)?;

        let _ = CloseHandle(sei.hProcess);

        Ok(exit_code)
    }
}
