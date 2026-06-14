use homepanel_agent::{state::AgentStateConfig, AgentState};
use tracing_subscriber::EnvFilter;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let _agent = AgentState::new(AgentStateConfig {
        scrollback_bytes: 10 * 1024 * 1024,
    });

    println!("homepanel-agent scaffold started");
}
