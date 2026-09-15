use anyhow::{Context, Result};
use rig_agent::{Agent as RigAgent, completion::Chat};
use rig_core::completion::Message as RigMessage;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

impl Message {
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
}

pub struct Agent {
    inner: RigAgent,
    history: Vec<RigMessage>,
}

impl Agent {
    pub fn new(inner: RigAgent) -> Self {
        Self {
            inner,
            history: Vec::new(),
        }
    }

    pub async fn reply(&mut self, input: impl Into<String>) -> Result<Message> {
        let content = self
            .inner
            .chat(input.into(), &mut self.history)
            .await
            .context("Rig Agent 对话失败")?;
        Ok(Message::new(Role::Assistant, content))
    }

    pub fn clear(&mut self) {
        self.history.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_message_accepts_owned_and_borrowed_text() {
        assert_eq!(Message::new(Role::User, "hello").content, "hello");
        assert_eq!(
            Message::new(Role::Assistant, String::from("hi")).content,
            "hi"
        );
    }
}
