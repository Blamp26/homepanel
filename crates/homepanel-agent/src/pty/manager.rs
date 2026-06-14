use crate::pty::session::{spawn_terminal, TerminalSession};
use homepanel_core::{
    error::{ApiError, Result},
    protocol::{ClientTerminalMessage, ServerTerminalMessage},
    types::{TerminalId, TerminalMetadata, TerminalRequest, TerminalStatus, TerminalSummary},
};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};
use tokio::sync::broadcast;
use tracing::{debug, warn};

pub struct TerminalAttachment {
    pub metadata: TerminalMetadata,
    pub scrollback: Vec<u8>,
    pub receiver: broadcast::Receiver<Vec<u8>>,
}

#[derive(Clone)]
pub struct TerminalSessionManager {
    sessions: Arc<RwLock<HashMap<TerminalId, Arc<TerminalSession>>>>,
    scrollback_bytes: usize,
}

impl TerminalSessionManager {
    pub fn new(scrollback_bytes: usize) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            scrollback_bytes,
        }
    }

    pub fn create_terminal(&self, request: TerminalRequest) -> Result<TerminalMetadata> {
        let id = TerminalId::new("terminal");
        let session = spawn_terminal(
            id.clone(),
            request.name,
            request.kind,
            request.command,
            request.cwd,
            request.shell,
            request.cols,
            request.rows,
            request.env,
            request.auto_restart,
            request.metadata_json,
            self.scrollback_bytes,
        )
        .map_err(ApiError::Io)?;
        let metadata = session.metadata();
        self.sessions
            .write()
            .expect("session lock")
            .insert(id, session);
        Ok(metadata)
    }

    pub fn list_terminals(&self) -> Vec<TerminalSummary> {
        let sessions = self.sessions.read().expect("session lock");
        let mut summaries = Vec::with_capacity(sessions.len());
        for session in sessions.values() {
            let _ = session.reconcile_status();
            let metadata = session.metadata();
            debug!(
                terminal_id = %metadata.id.0,
                status = ?metadata.status,
                "listed terminal session",
            );
            summaries.push(metadata.into());
        }
        summaries
    }

    pub fn get_terminal(&self, id: &TerminalId) -> Option<TerminalMetadata> {
        self.sessions
            .read()
            .expect("session lock")
            .get(id)
            .map(|session| {
                let _ = session.reconcile_status();
                session.metadata()
            })
    }

    pub fn attach_terminal(&self, id: &TerminalId) -> Result<TerminalAttachment> {
        let session = self
            .sessions
            .read()
            .expect("session lock")
            .get(id)
            .cloned()
            .ok_or(ApiError::NotFound)?;
        let exited = session.reconcile_status();
        let metadata = session.metadata();
        debug!(
            terminal_id = %metadata.id.0,
            status = ?metadata.status,
            exited = ?exited.map(|status| status.exit_code()),
            "attach terminal requested",
        );
        if !session.is_attachable() {
            warn!(
                terminal_id = %metadata.id.0,
                status = ?metadata.status,
                "attach requested for unavailable terminal",
            );
            return Err(ApiError::NotFound);
        }
        session.mark_attached();
        let metadata = session.metadata();
        debug!(
            terminal_id = %metadata.id.0,
            status = ?metadata.status,
            "attach terminal succeeded",
        );
        Ok(TerminalAttachment {
            metadata,
            scrollback: session.snapshot_scrollback(),
            receiver: session.broadcaster.subscribe(),
        })
    }

    pub fn write_input(&self, id: &TerminalId, data: &[u8]) -> Result<()> {
        let session = self
            .sessions
            .read()
            .expect("session lock")
            .get(id)
            .cloned()
            .ok_or(ApiError::NotFound)?;
        let mut writer = session.writer.lock().expect("writer lock");
        writer.write_all(data).map_err(ApiError::Io)?;
        writer.flush().map_err(ApiError::Io)?;
        Ok(())
    }

    pub fn resize_terminal(&self, id: &TerminalId, cols: u16, rows: u16) -> Result<()> {
        let session = self
            .sessions
            .read()
            .expect("session lock")
            .get(id)
            .cloned()
            .ok_or(ApiError::NotFound)?;
        session.resize(cols, rows).map_err(ApiError::Io)
    }

    pub fn kill_terminal(&self, id: &TerminalId) -> Result<()> {
        let session = self
            .sessions
            .read()
            .expect("session lock")
            .get(id)
            .cloned()
            .ok_or(ApiError::NotFound)?;
        let metadata = session.metadata();
        debug!(
            terminal_id = %metadata.id.0,
            status = ?metadata.status,
            "kill terminal requested",
        );
        session.kill().map_err(ApiError::Io)?;
        session.update_status(TerminalStatus::Exited, Some(0));
        debug!(
            terminal_id = %metadata.id.0,
            "kill terminal succeeded",
        );
        Ok(())
    }

    pub fn clear_scrollback(&self, id: &TerminalId) -> Result<()> {
        let session = self
            .sessions
            .read()
            .expect("session lock")
            .get(id)
            .cloned()
            .ok_or(ApiError::NotFound)?;
        if let Ok(mut buffer) = session.scrollback.lock() {
            buffer.clear();
        }
        Ok(())
    }

    pub fn handle_client_message(
        &self,
        id: &TerminalId,
        message: ClientTerminalMessage,
    ) -> Result<Option<ServerTerminalMessage>> {
        match message {
            ClientTerminalMessage::Input { data } => {
                self.write_input(id, data.as_bytes())?;
                Ok(None)
            }
            ClientTerminalMessage::Resize { cols, rows } => {
                self.resize_terminal(id, cols, rows)?;
                Ok(None)
            }
            ClientTerminalMessage::Ping => Ok(Some(ServerTerminalMessage::Status {
                status: self
                    .get_terminal(id)
                    .map(|terminal| terminal.status)
                    .unwrap_or(TerminalStatus::Failed),
            })),
            ClientTerminalMessage::ClearScrollback => {
                self.clear_scrollback(id)?;
                Ok(None)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::BTreeMap, thread, time::Duration};

    fn shell_request(command: &str, name: &str) -> TerminalRequest {
        TerminalRequest {
            name: name.to_string(),
            kind: homepanel_core::types::TerminalKind::Shell,
            command: command.to_string(),
            cwd: std::env::temp_dir(),
            cols: 80,
            rows: 24,
            env: BTreeMap::new(),
            shell: Some("/bin/bash".to_string()),
            auto_restart: false,
            metadata_json: None,
        }
    }

    #[test]
    fn killing_one_terminal_does_not_break_attach_to_another() {
        let manager = TerminalSessionManager::new(4096);
        let first = manager
            .create_terminal(shell_request("sleep 5", "first"))
            .expect("first terminal");
        let second = manager
            .create_terminal(shell_request("sleep 5", "second"))
            .expect("second terminal");

        assert!(manager.attach_terminal(&first.id).is_ok());
        assert!(manager.attach_terminal(&second.id).is_ok());

        manager.kill_terminal(&second.id).expect("kill second");
        thread::sleep(Duration::from_millis(50));

        assert!(manager.attach_terminal(&first.id).is_ok());
        assert!(matches!(manager.attach_terminal(&second.id), Err(ApiError::NotFound)));
    }

    #[test]
    fn exited_terminal_is_reconciled_before_attach() {
        let manager = TerminalSessionManager::new(4096);
        let terminal = manager
            .create_terminal(shell_request("exit 0", "short-lived"))
            .expect("terminal");

        thread::sleep(Duration::from_millis(200));
        let summaries = manager.list_terminals();
        assert_eq!(summaries.len(), 1);
        assert!(matches!(summaries[0].status, TerminalStatus::Exited));
        assert!(matches!(manager.attach_terminal(&terminal.id), Err(ApiError::NotFound)));
    }
}
