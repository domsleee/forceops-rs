//! Process termination utilities

use crate::lock_checker::ProcessInfo;
use std::process;
use tracing::warn;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, PROCESS_TERMINATE, TerminateProcess,
};

/// Kills the specified processes.
///
/// Skips the current process and handles errors gracefully.
pub fn kill_processes(processes: &[ProcessInfo]) {
    let current_pid = unsafe { GetCurrentProcessId() };

    for process_info in processes {
        if process_info.process_id == current_pid {
            continue;
        }

        if let Err(e) = kill_process(process_info.process_id) {
            warn!("Failed to kill process {}: {}", process_info.process_id, e);
        }
    }
}

fn kill_process(pid: u32) -> Result<(), String> {
    unsafe {
        let handle: HANDLE =
            OpenProcess(PROCESS_TERMINATE, false, pid).map_err(|e| format!("{}", e))?;

        if handle.is_invalid() {
            return Err("Failed to open process".to_string());
        }

        let result = TerminateProcess(handle, 1);
        let _ = CloseHandle(handle);

        if result.is_err() {
            return Err(result.unwrap_err().to_string());
        }

        Ok(())
    }
}

/// Gets the current process ID.
pub fn current_process_id() -> u32 {
    process::id()
}
