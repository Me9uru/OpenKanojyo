use futures_util::StreamExt;
use rig_agent::{
    Agent as RigAgent,
    agent::MultiTurnStreamItem,
    streaming::{StreamedAssistantContent, StreamingChat},
};
use rig_core::completion::Message as RigMessage;

use crate::error::AppError;

pub enum StreamUpdate {
    Text(String),
    Reset,
}

/// Rig 流式对话及其进程内历史。
pub struct Conversation {
    inner: RigAgent,
    history: Vec<RigMessage>,
}

impl Conversation {
    pub fn new(inner: RigAgent) -> Self {
        Self {
            inner,
            history: Vec::new(),
        }
    }

    pub async fn reply(
        &mut self,
        input: impl Into<String>,
        mut on_update: impl FnMut(StreamUpdate),
    ) -> Result<String, AppError> {
        let mut stream = self
            .inner
            .stream_chat(input.into(), self.history.clone())
            .await;
        let mut final_response = None;

        while let Some(item) = stream.next().await {
            match item.map_err(|source| AppError::ModelRequest { source })? {
                MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::Text(text)) => {
                    on_update(StreamUpdate::Text(text.text))
                }
                MultiTurnStreamItem::ModelTurnRetried { .. } => on_update(StreamUpdate::Reset),
                MultiTurnStreamItem::FinalResponse(response) => {
                    final_response = Some(response);
                }
                _ => {}
            }
        }

        let response = final_response.ok_or(AppError::MissingFinalResponse)?;
        if let Some(messages) = response.messages() {
            self.history = messages.to_vec();
        }
        Ok(response.output().to_owned())
    }

    pub fn clear(&mut self) {
        self.history.clear();
    }
}
