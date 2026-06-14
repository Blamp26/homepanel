use crate::types::TerminalStatus;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientTerminalMessage {
    Input { data: String },
    Resize { cols: u16, rows: u16 },
    Ping,
    ClearScrollback,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerTerminalMessage {
    Hello { terminal_id: String, status: TerminalStatus },
    Scrollback { data: String },
    Output { data: String },
    Status { status: TerminalStatus },
    Exit { code: Option<i32> },
    Error { message: String },
}

#[cfg(test)]
mod tests {
    use super::{ClientTerminalMessage, ServerTerminalMessage};
    use crate::types::TerminalStatus;

    #[test]
    fn protocol_round_trip() {
        let message = ClientTerminalMessage::Resize { cols: 120, rows: 32 };
        let encoded = serde_json::to_string(&message).expect("serialize");
        let decoded: ClientTerminalMessage = serde_json::from_str(&encoded).expect("deserialize");
        match decoded {
            ClientTerminalMessage::Resize { cols, rows } => {
                assert_eq!((cols, rows), (120, 32));
            }
            _ => panic!("wrong variant"),
        }

        let server = ServerTerminalMessage::Status {
            status: TerminalStatus::Running,
        };
        let encoded = serde_json::to_string(&server).expect("serialize");
        assert!(encoded.contains("running"));
    }
}
