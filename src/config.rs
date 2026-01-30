/// Configuration for ForceOps operations
#[derive(Debug, Clone)]
pub struct ForceOpsConfig {
    /// The number of retries when performing an operation.
    /// For example, five retries equals six total attempts.
    pub max_retries: u32,

    /// The time to wait in milliseconds before retrying the operation.
    pub retry_delay_ms: u64,

    /// Whether to disable auto-elevation when permission errors occur.
    pub disable_elevate: bool,
}

impl Default for ForceOpsConfig {
    fn default() -> Self {
        Self {
            max_retries: 10,
            retry_delay_ms: 50,
            disable_elevate: false,
        }
    }
}
