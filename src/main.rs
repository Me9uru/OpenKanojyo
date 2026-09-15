use anyhow::Result;
use rig_agent::AgentBuilder;
use rig_core::{client::CompletionClient, providers::openai};
use tokio::sync::mpsc;

use crate::{agent::Agent, config::Config, ui::App};

mod agent;
mod config;
mod ui;

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::load()?;
    let provider_label = format!("{} @ {}", config.model, config.base_url);
    let client = openai::CompletionsClient::builder()
        .api_key(config.api_key)
        .base_url(config.base_url)
        .build()?;
    let model = client.completion_model(config.model);
    let agent = Agent::new(
        AgentBuilder::new(model)
            .name("open_kanojyo")
            .description("OpenKanojyo TUI 对话助手")
            .preamble(&config.preamble)
            .temperature(config.temperature)
            .build(),
    );

    let (command_tx, command_rx) = mpsc::unbounded_channel();
    let (event_tx, event_rx) = mpsc::unbounded_channel();
    tokio::spawn(ui::agent_worker(agent, command_rx, event_tx));

    App::new(provider_label, command_tx, event_rx).run()
}
