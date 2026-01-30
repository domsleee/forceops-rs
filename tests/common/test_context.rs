//! Test context with mock implementations

use fops::config::ForceOpsConfig;
use std::sync::{Arc, Mutex};

/// Fake logger that captures log messages for testing
#[derive(Clone, Default)]
pub struct FakeLogger {
    logs: Arc<Mutex<Vec<String>>>,
}

impl FakeLogger {
    pub fn new() -> Self {
        Self {
            logs: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn log(&self, message: &str) {
        let mut logs = self.logs.lock().unwrap();
        logs.push(message.to_string());
    }

    pub fn get_all_logs_string(&self) -> String {
        let logs = self.logs.lock().unwrap();
        logs.join("\n")
    }
}

/// Test context with configurable components
pub struct TestContext {
    pub config: ForceOpsConfig,
    pub logger: FakeLogger,
    pub is_elevated: bool,
    pub relaunch_exit_code: i32,
}

impl TestContext {
    pub fn new() -> Self {
        Self {
            config: ForceOpsConfig::default(),
            logger: FakeLogger::new(),
            is_elevated: false,
            relaunch_exit_code: 1,
        }
    }
}

impl Default for TestContext {
    fn default() -> Self {
        Self::new()
    }
}
