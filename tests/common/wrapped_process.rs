//! Wrapped process that gets killed on drop (RAII pattern)

use std::process::{Child, Command, Stdio};

/// A wrapper around a process that kills it when dropped
pub struct WrappedProcess {
    pub process: Child,
}

impl WrappedProcess {
    pub fn new(process: Child) -> Self {
        Self { process }
    }
}

impl Drop for WrappedProcess {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}
