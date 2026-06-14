pub mod config;
pub mod error;
pub mod protocol;
pub mod scrollback;
pub mod types;

pub use config::{AppConfig, ConfigError, load_config};
pub use error::{ApiError, Result};
pub use protocol::{ClientTerminalMessage, ServerTerminalMessage};
pub use scrollback::ScrollbackBuffer;
pub use types::{
    AuditEvent, FileAllowlist, LogSource, SessionId, SettingsEntry, TerminalId, TerminalKind,
    TerminalMetadata, TerminalRequest, TerminalStatus, TerminalSummary, UserRole,
};
