//! Lock detection using Windows Restart Manager API and NtQuerySystemInformation
//!
//! This module provides functionality to detect which processes are holding locks
//! on files or directories using:
//! - Windows Restart Manager API (for files)
//! - NtQueryInformationFile with FileProcessIdsUsingFileInformation (for files, low-level)
//! - Process enumeration with PEB reading (for directories)

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use thiserror::Error;
use windows::Win32::Foundation::{
    CloseHandle, HANDLE, MAX_PATH, NTSTATUS, UNICODE_STRING, WIN32_ERROR,
};
use windows::Win32::System::ProcessStatus::EnumProcesses;
use windows::Win32::System::RestartManager::{
    CCH_RM_SESSION_KEY, RM_PROCESS_INFO, RmEndSession, RmGetList, RmRegisterResources,
    RmStartSession,
};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_FORMAT, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
    QueryFullProcessImageNameW,
};
use windows::core::{PCWSTR, PWSTR};

#[derive(Error, Debug)]
pub enum LockCheckError {
    #[error("Failed to start Restart Manager session: {0}")]
    SessionStart(windows::core::Error),

    #[error("Failed to register resources: {0}")]
    RegisterResources(windows::core::Error),

    #[error("Failed to get list (RmGetList() error {code}): {message}")]
    GetList { code: u32, message: String },

    #[error("File not found: {0}")]
    FileNotFound(String),
}

/// Information about a process holding a lock
#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub process_id: u32,
    pub executable_name: Option<String>,
    pub application_name: Option<String>,
}

// Link to ntdll for NtQueryInformationProcess
#[link(name = "ntdll")]
unsafe extern "system" {
    fn NtQueryInformationProcess(
        process_handle: HANDLE,
        process_information_class: u32,
        process_information: *mut std::ffi::c_void,
        process_information_length: u32,
        return_length: *mut u32,
    ) -> NTSTATUS;
}

const PROCESS_BASIC_INFORMATION_CLASS: u32 = 0;

#[repr(C)]
struct ProcessBasicInformation {
    exit_status: i32,
    peb_base_address: *mut std::ffi::c_void,
    affinity_mask: usize,
    base_priority: i32,
    unique_process_id: usize,
    inherited_from_unique_process_id: usize,
}

/// Get processes locking the specified files using Restart Manager API.
pub fn get_locking_processes(paths: &[&Path]) -> Result<Vec<ProcessInfo>, LockCheckError> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }

    unsafe {
        let mut session_handle: u32 = 0;
        let mut session_key = [0u16; CCH_RM_SESSION_KEY as usize + 1];

        let result = RmStartSession(&mut session_handle, None, PWSTR(session_key.as_mut_ptr()));
        if result.0 != 0 {
            return Err(LockCheckError::SessionStart(
                windows::core::Error::from_thread(),
            ));
        }

        let _guard = scopeguard::guard(session_handle, |handle| {
            let _ = RmEndSession(handle);
        });

        let wide_paths: Vec<Vec<u16>> = paths
            .iter()
            .map(|p| {
                OsStr::new(p)
                    .encode_wide()
                    .chain(std::iter::once(0))
                    .collect()
            })
            .collect();

        let path_ptrs: Vec<PCWSTR> = wide_paths.iter().map(|p| PCWSTR(p.as_ptr())).collect();

        let result = RmRegisterResources(session_handle, Some(&path_ptrs), None, None);
        if result.0 != 0 {
            return Err(LockCheckError::RegisterResources(
                windows::core::Error::from_thread(),
            ));
        }

        let mut needed: u32 = 0;
        let mut count: u32 = 0;
        let mut reboot_reasons: u32 = 0;

        let result = RmGetList(
            session_handle,
            &mut needed,
            &mut count,
            None,
            &mut reboot_reasons,
        );

        const ERROR_MORE_DATA: WIN32_ERROR = WIN32_ERROR(234);
        const ERROR_SUCCESS: WIN32_ERROR = WIN32_ERROR(0);
        const ERROR_ACCESS_DENIED: WIN32_ERROR = WIN32_ERROR(5);

        if result != ERROR_SUCCESS && result != ERROR_MORE_DATA {
            let message = if result == ERROR_ACCESS_DENIED {
                "Access is denied.".to_string()
            } else {
                format!("Error code {}", result.0)
            };
            return Err(LockCheckError::GetList {
                code: result.0,
                message,
            });
        }

        if needed == 0 {
            return Ok(Vec::new());
        }

        let mut process_info: Vec<RM_PROCESS_INFO> =
            vec![RM_PROCESS_INFO::default(); needed as usize];
        count = needed;

        let result = RmGetList(
            session_handle,
            &mut needed,
            &mut count,
            Some(process_info.as_mut_ptr()),
            &mut reboot_reasons,
        );

        if result != ERROR_SUCCESS && result != ERROR_MORE_DATA {
            let message = if result == ERROR_ACCESS_DENIED {
                "Access is denied.".to_string()
            } else {
                format!("Error code {}", result.0)
            };
            return Err(LockCheckError::GetList {
                code: result.0,
                message,
            });
        }

        let processes: Vec<ProcessInfo> = process_info
            .into_iter()
            .take(count as usize)
            .map(|info| {
                let app_name = wide_to_string(&info.strAppName);
                let service_name = wide_to_string(&info.strServiceShortName);

                let exe_name = if service_name.as_ref().map(|s| s.is_empty()).unwrap_or(true) {
                    app_name.clone()
                } else {
                    service_name
                };

                ProcessInfo {
                    process_id: info.Process.dwProcessId,
                    executable_name: exe_name.or_else(|| app_name.clone()),
                    application_name: app_name,
                }
            })
            .collect();

        Ok(processes)
    }
}

/// Get processes whose working directory is within the target directory.
/// This is used for directory lock detection.
pub fn get_locking_processes_low_level(path: &Path) -> Result<Vec<ProcessInfo>, LockCheckError> {
    let target_path = std::fs::canonicalize(path).map_err(|_| {
        LockCheckError::FileNotFound(format!("Cannot canonicalize '{}'", path.display()))
    })?;
    let target_path_str = target_path.to_string_lossy().to_lowercase();
    // Remove \\?\ prefix if present
    let target_path_clean = target_path_str
        .strip_prefix(r"\\?\")
        .unwrap_or(&target_path_str)
        .to_string();

    unsafe {
        // Enumerate all processes
        let mut pids = [0u32; 4096];
        let mut bytes_returned: u32 = 0;

        if EnumProcesses(
            pids.as_mut_ptr(),
            (pids.len() * std::mem::size_of::<u32>()) as u32,
            &mut bytes_returned,
        )
        .is_err()
        {
            return Ok(Vec::new());
        }

        let num_processes = bytes_returned as usize / std::mem::size_of::<u32>();
        let current_pid = std::process::id();
        let mut found_processes: Vec<ProcessInfo> = Vec::new();

        for &pid in &pids[..num_processes] {
            if pid == 0 || pid == current_pid {
                continue;
            }

            // Try to get the process's current working directory
            if let Some(cwd) = get_process_current_directory(pid) {
                let cwd_lower = cwd.to_lowercase();
                let cwd_clean = cwd_lower.strip_prefix(r"\\?\").unwrap_or(&cwd_lower);

                // Check if the process's CWD starts with our target directory
                if cwd_clean.starts_with(&target_path_clean)
                    || target_path_clean.starts_with(cwd_clean)
                {
                    let exe_path = get_process_exe_path(pid);
                    found_processes.push(ProcessInfo {
                        process_id: pid,
                        executable_name: exe_path.clone(),
                        application_name: exe_path,
                    });
                }
            }
        }

        Ok(found_processes)
    }
}

/// Get the current working directory of a process by reading its PEB
fn get_process_current_directory(pid: u32) -> Option<String> {
    use windows::Win32::System::Diagnostics::Debug::ReadProcessMemory;

    unsafe {
        // Open the process with read access
        let process_handle =
            OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid).ok()?;

        let _guard = scopeguard::guard(process_handle, |h| {
            let _ = CloseHandle(h);
        });

        // Get the PEB address
        let mut pbi = ProcessBasicInformation {
            exit_status: 0,
            peb_base_address: std::ptr::null_mut(),
            affinity_mask: 0,
            base_priority: 0,
            unique_process_id: 0,
            inherited_from_unique_process_id: 0,
        };
        let mut return_length: u32 = 0;

        let status = NtQueryInformationProcess(
            process_handle,
            PROCESS_BASIC_INFORMATION_CLASS,
            &mut pbi as *mut _ as *mut _,
            std::mem::size_of::<ProcessBasicInformation>() as u32,
            &mut return_length,
        );

        if status.is_err() || pbi.peb_base_address.is_null() {
            return None;
        }

        // Read the PEB to get RTL_USER_PROCESS_PARAMETERS pointer
        // PEB layout (64-bit): offset 0x20 contains ProcessParameters pointer
        // PEB layout (32-bit): offset 0x10 contains ProcessParameters pointer
        #[cfg(target_pointer_width = "64")]
        const PROCESS_PARAMETERS_OFFSET: usize = 0x20;
        #[cfg(target_pointer_width = "32")]
        const PROCESS_PARAMETERS_OFFSET: usize = 0x10;

        let mut process_parameters_ptr: usize = 0;
        let mut bytes_read: usize = 0;

        let result = ReadProcessMemory(
            process_handle,
            (pbi.peb_base_address as usize + PROCESS_PARAMETERS_OFFSET) as *const _,
            &mut process_parameters_ptr as *mut _ as *mut _,
            std::mem::size_of::<usize>(),
            Some(&mut bytes_read),
        );

        if result.is_err() || process_parameters_ptr == 0 {
            return None;
        }

        // Read the CurrentDirectory from RTL_USER_PROCESS_PARAMETERS
        // CurrentDirectory is a CURDIR structure at offset 0x38 (64-bit) or 0x24 (32-bit)
        // CURDIR contains UNICODE_STRING at the start
        #[cfg(target_pointer_width = "64")]
        const CURRENT_DIRECTORY_OFFSET: usize = 0x38;
        #[cfg(target_pointer_width = "32")]
        const CURRENT_DIRECTORY_OFFSET: usize = 0x24;

        let mut unicode_string = UNICODE_STRING::default();
        let result = ReadProcessMemory(
            process_handle,
            (process_parameters_ptr + CURRENT_DIRECTORY_OFFSET) as *const _,
            &mut unicode_string as *mut _ as *mut _,
            std::mem::size_of::<UNICODE_STRING>(),
            Some(&mut bytes_read),
        );

        if result.is_err() || unicode_string.Length == 0 || unicode_string.Buffer.is_null() {
            return None;
        }

        // Read the actual string
        let len = (unicode_string.Length / 2) as usize;
        let mut buffer: Vec<u16> = vec![0; len];

        let result = ReadProcessMemory(
            process_handle,
            unicode_string.Buffer.0 as *const _,
            buffer.as_mut_ptr() as *mut _,
            unicode_string.Length as usize,
            Some(&mut bytes_read),
        );

        if result.is_err() {
            return None;
        }

        String::from_utf16(&buffer).ok()
    }
}

fn get_process_exe_path(pid: u32) -> Option<String> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid).ok()?;

        let mut buffer = [0u16; MAX_PATH as usize];
        let mut size = buffer.len() as u32;

        let result = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_FORMAT(0),
            PWSTR(buffer.as_mut_ptr()),
            &mut size,
        );

        let _ = CloseHandle(handle);

        if result.is_ok() {
            wide_to_string(&buffer[..size as usize])
        } else {
            None
        }
    }
}

/// Get locks on a file or directory.
/// For directories, uses process enumeration with PEB reading.
/// For files, uses Restart Manager API.
pub fn get_locks(path: &Path) -> Result<Vec<ProcessInfo>, LockCheckError> {
    if !path.exists() {
        return Err(LockCheckError::FileNotFound(format!(
            "Cannot list locks of '{}'. No such file or directory",
            path.display()
        )));
    }

    if path.is_dir() {
        get_locking_processes_low_level(path)
    } else {
        get_locking_processes(&[path])
    }
}

fn wide_to_string(wide: &[u16]) -> Option<String> {
    let len = wide.iter().position(|&c| c == 0).unwrap_or(wide.len());
    if len == 0 {
        return None;
    }
    String::from_utf16(&wide[..len]).ok()
}

mod scopeguard {
    pub struct ScopeGuard<T, F: FnOnce(T)> {
        value: Option<T>,
        dropfn: Option<F>,
    }

    impl<T, F: FnOnce(T)> Drop for ScopeGuard<T, F> {
        fn drop(&mut self) {
            if let (Some(value), Some(dropfn)) = (self.value.take(), self.dropfn.take()) {
                dropfn(value);
            }
        }
    }

    pub fn guard<T, F: FnOnce(T)>(value: T, dropfn: F) -> ScopeGuard<T, F> {
        ScopeGuard {
            value: Some(value),
            dropfn: Some(dropfn),
        }
    }
}
