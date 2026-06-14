use homepanel_agent::AgentState;
use homepanel_core::config::AppConfig;
use sqlx::SqlitePool;
use std::sync::{Arc, RwLock};

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub agent: AgentState,
    pub db: SqlitePool,
    pub sessions: Arc<RwLock<std::collections::HashMap<String, SessionRecord>>>,
}

#[derive(Clone, Debug)]
pub struct SessionRecord {
    pub user_id: String,
    pub username: String,
    pub token_hash: String,
}

impl AppState {
    pub fn new(config: AppConfig, db: SqlitePool) -> Self {
        Self {
            agent: AgentState::new(homepanel_agent::state::AgentStateConfig {
                scrollback_bytes: config.agent.terminal_scrollback_bytes,
            }),
            config,
            db,
            sessions: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }
}
