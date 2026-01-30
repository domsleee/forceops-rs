//! ForceOps - Forcefully perform file operations by terminating processes holding locks
//!
//! This crate provides functionality to delete files and directories even when they are
//! locked by other processes, by detecting and terminating those processes.

pub mod cli;
pub mod config;
pub mod deleter;
pub mod elevation;
pub mod lock_checker;
pub mod process;
pub mod utils;

pub use config::ForceOpsConfig;
pub use deleter::FileAndDirectoryDeleter;
pub use lock_checker::{ProcessInfo, get_locking_processes, get_locking_processes_low_level};
