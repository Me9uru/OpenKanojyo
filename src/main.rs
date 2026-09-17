use anyhow::Result;
use open_kanojyo::{App, AssembledAgent, Config, Conversation, application_worker, assemble_agent};
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<()> {
    let AssembledAgent {
        agent,
        provider_label,
        skill_summaries,
        tool_summaries,
    } = assemble_agent(Config::load()?)?;
    let conversation = Conversation::new(agent);

    let (command_tx, command_rx) = mpsc::unbounded_channel();
    let (event_tx, event_rx) = mpsc::unbounded_channel();
    let worker = tokio::spawn(application_worker(conversation, command_rx, event_tx));

    let result = App::new(provider_label, command_tx, event_rx)
        .with_capabilities(skill_summaries, tool_summaries)
        .run();
    worker.abort();
    let _ = worker.await;
    result
}
