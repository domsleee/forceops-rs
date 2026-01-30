//! Stdout capture utilities for tests

use std::sync::{Arc, Mutex};

/// Captured stdout content
pub struct CapturedStdout {
    content: Arc<Mutex<Vec<u8>>>,
}

impl CapturedStdout {
    pub fn new() -> Self {
        Self {
            content: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn get_content(&self) -> String {
        let content = self.content.lock().unwrap();
        String::from_utf8_lossy(&content).to_string()
    }
}

impl Default for CapturedStdout {
    fn default() -> Self {
        Self::new()
    }
}
