use homepanel_core::protocol::{ClientTerminalMessage, ServerTerminalMessage};

pub fn encode_server_message(message: &ServerTerminalMessage) -> serde_json::Result<String> {
    serde_json::to_string(message)
}

pub fn decode_client_message(raw: &str) -> serde_json::Result<ClientTerminalMessage> {
    serde_json::from_str(raw)
}
