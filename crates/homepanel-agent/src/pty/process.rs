use homepanel_core::types::TerminalStatus;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct ProcessState {
    pub status: TerminalStatus,
    pub exit_code: Option<i32>,
    pub last_heartbeat: Option<std::time::Instant>,
}

impl ProcessState {
    pub fn running() -> Self {
        Self {
            status: TerminalStatus::Running,
            exit_code: None,
            last_heartbeat: Some(std::time::Instant::now()),
        }
    }
}

pub fn backoff_delay(attempt: u32) -> Duration {
    let millis = 250u64.saturating_mul(2u64.saturating_pow(attempt.min(5)));
    Duration::from_millis(millis.min(5_000))
}
