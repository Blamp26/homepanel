use crate::pty::scrollback::ScrollbackBuffer;
use chrono::Utc;
use homepanel_core::types::{TerminalId, TerminalKind, TerminalMetadata, TerminalStatus};
use portable_pty::{Child, CommandBuilder, ExitStatus, MasterPty, NativePtySystem, PtySize, PtySystem};
use std::{
    collections::BTreeMap,
    convert::TryFrom,
    io::{Read, Write},
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};
use tokio::sync::broadcast;
use tracing::debug;

fn io_error_from_pty_error(err: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::other(err.to_string())
}

pub struct TerminalSession {
    pub metadata: Arc<Mutex<TerminalMetadata>>,
    pub scrollback: Arc<Mutex<ScrollbackBuffer>>,
    pub broadcaster: broadcast::Sender<Vec<u8>>,
    pub writer: Arc<Mutex<Box<dyn Write + Send>>>,
    master: Arc<Mutex<Box<dyn MasterPty + Send>>>,
    child: Arc<Mutex<Box<dyn Child + Send>>>,
}

impl TerminalSession {
    pub fn snapshot_scrollback(&self) -> Vec<u8> {
        self.scrollback.lock().expect("scrollback lock").snapshot()
    }

    pub fn metadata(&self) -> TerminalMetadata {
        self.metadata.lock().expect("metadata lock").clone()
    }

    pub fn mark_attached(&self) {
        if let Ok(mut metadata) = self.metadata.lock() {
            metadata.last_attached_at = Some(Utc::now());
            metadata.updated_at = Utc::now();
            if metadata.status == TerminalStatus::Starting {
                metadata.status = TerminalStatus::Running;
            }
        }
    }

    pub fn update_status(&self, status: TerminalStatus, exit_code: Option<i32>) {
        if let Ok(mut metadata) = self.metadata.lock() {
            metadata.status = status;
            metadata.exit_code = exit_code;
            metadata.updated_at = Utc::now();
        }
    }

    pub fn reconcile_status(&self) -> Option<ExitStatus> {
        let status = {
            let mut child = self.child.lock().expect("child lock");
            child.try_wait().ok().flatten()
        };

        if let Some(ref exit_status) = status {
            let exit_code = i32::try_from(exit_status.exit_code()).ok();
            self.update_status(TerminalStatus::Exited, exit_code);
        }

        status
    }

    pub fn is_attachable(&self) -> bool {
        !matches!(
            self.metadata().status,
            TerminalStatus::Exited | TerminalStatus::Failed
        )
    }

    pub fn kill(&self) -> std::io::Result<()> {
        let mut child = self.child.lock().expect("child lock");
        child.kill()
    }

    pub fn resize(&self, cols: u16, rows: u16) -> std::io::Result<()> {
        let master = self.master.lock().expect("master lock");
        master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(io_error_from_pty_error)
    }
}

pub fn spawn_terminal(
    id: TerminalId,
    name: String,
    kind: TerminalKind,
    command: String,
    cwd: PathBuf,
    shell: Option<String>,
    cols: u16,
    rows: u16,
    env: BTreeMap<String, String>,
    auto_restart: bool,
    metadata_json: Option<serde_json::Value>,
    scrollback_bytes: usize,
) -> std::io::Result<Arc<TerminalSession>> {
    let pty_system = NativePtySystem::default();
    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(io_error_from_pty_error)?;

    let shell_path = shell.clone().unwrap_or_else(|| "/bin/bash".to_string());
    let mut builder = CommandBuilder::new(shell_path);
    builder.arg("-lc");
    builder.arg(command.clone());
    builder.cwd(cwd.clone());
    for (key, value) in env {
        builder.env(key, value);
    }

    let master = pair.master;
    let child = pair
        .slave
        .spawn_command(builder)
        .map_err(io_error_from_pty_error)?;
    let writer = master.take_writer().map_err(io_error_from_pty_error)?;
    let reader = master.try_clone_reader().map_err(io_error_from_pty_error)?;
    let (broadcaster, _) = broadcast::channel(64);
    let scrollback = Arc::new(Mutex::new(ScrollbackBuffer::new(scrollback_bytes)));

    let metadata = TerminalMetadata {
        id,
        name,
        kind,
        status: TerminalStatus::Starting,
        command,
        cwd,
        shell,
        cols,
        rows,
        auto_restart,
        exit_code: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        last_attached_at: None,
        metadata_json,
    };

    let session = Arc::new(TerminalSession {
        metadata: Arc::new(Mutex::new(metadata)),
        scrollback: scrollback.clone(),
        broadcaster: broadcaster.clone(),
        writer: Arc::new(Mutex::new(writer)),
        master: Arc::new(Mutex::new(master)),
        child: Arc::new(Mutex::new(child)),
    });

    debug!(
        terminal_id = %session.metadata().id.0,
        terminal_name = %session.metadata().name,
        command = %session.metadata().command,
        status = ?session.metadata().status,
        "spawned terminal session",
    );

    let session_for_reader = Arc::clone(&session);
    std::thread::spawn(move || {
        let mut reader = reader;
        let mut buffer = [0u8; 8192];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => {
                    let exit_code = {
                        let mut child = session_for_reader.child.lock().expect("child lock");
                        let mut code = None;
                        for _ in 0..10 {
                            match child.try_wait() {
                                Ok(Some(status)) => {
                                    code = i32::try_from(status.exit_code()).ok();
                                    break;
                                }
                                Ok(None) => thread::sleep(Duration::from_millis(20)),
                                Err(_) => break,
                            }
                        }
                        code
                    };
                    session_for_reader.update_status(
                        TerminalStatus::Exited,
                        exit_code.or(Some(0)),
                    );
                    let _ = broadcaster.send(Vec::new());
                    break;
                }
                Ok(size) => {
                    let bytes = buffer[..size].to_vec();
                    if let Ok(mut guard) = session_for_reader.scrollback.lock() {
                        guard.append(&bytes);
                    }
                    session_for_reader.update_status(TerminalStatus::Running, None);
                    let _ = broadcaster.send(bytes);
                }
                Err(_) => {
                    session_for_reader.update_status(TerminalStatus::Failed, None);
                    break;
                }
            }
        }
    });

    Ok(session)
}
