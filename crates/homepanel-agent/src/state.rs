use crate::pty::manager::TerminalSessionManager;
use homepanel_core::types::TerminalId;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct AgentStateConfig {
    pub scrollback_bytes: usize,
}

#[derive(Clone)]
pub struct AgentState {
    manager: Arc<TerminalSessionManager>,
}

impl AgentState {
    pub fn new(config: AgentStateConfig) -> Self {
        Self {
            manager: Arc::new(TerminalSessionManager::new(config.scrollback_bytes)),
        }
    }

    pub fn manager(&self) -> Arc<TerminalSessionManager> {
        self.manager.clone()
    }

    pub fn get_terminal(&self, id: &TerminalId) -> Option<homepanel_core::types::TerminalMetadata> {
        self.manager.get_terminal(id)
    }
}
