use std::collections::VecDeque;

use tokio::sync::mpsc;

use super::protocol::{ApplicationCommand, ApplicationEvent, ConversationEvent};
use crate::{
    conversation::{Conversation, StreamUpdate},
    error::AppError,
};

pub async fn application_worker(
    mut conversation: Conversation,
    mut commands: mpsc::UnboundedReceiver<ApplicationCommand>,
    events: mpsc::UnboundedSender<ApplicationEvent>,
) {
    // 接受其他客户端连续提交的命令；TUI 自行缓冲尚未提交的输入。
    let mut pending = VecDeque::new();

    loop {
        let command = match pending.pop_front() {
            Some(command) => command,
            None => {
                tokio::select! {
                    _ = events.closed() => return,
                    command = commands.recv() => match command {
                        Some(command) => command,
                        None => return,
                    },
                }
            }
        };

        match command {
            ApplicationCommand::Clear => {
                conversation.clear();
                pending.clear();
            }
            ApplicationCommand::Chat { generation, input } => {
                match drive_reply(
                    &mut conversation,
                    &mut commands,
                    &events,
                    &mut pending,
                    generation,
                    input,
                )
                .await
                {
                    ReplyOutcome::Stopped => return,
                    ReplyOutcome::Cleared => {
                        conversation.clear();
                        pending.clear();
                    }
                    ReplyOutcome::Completed(result) => {
                        let kind = match result {
                            Ok(message) => ConversationEvent::Reply(message),
                            Err(error) => ConversationEvent::Error(error.user_facing()),
                        };
                        if events.send(ApplicationEvent { generation, kind }).is_err() {
                            return;
                        }
                    }
                }
            }
        }
    }
}

async fn drive_reply(
    conversation: &mut Conversation,
    commands: &mut mpsc::UnboundedReceiver<ApplicationCommand>,
    events: &mpsc::UnboundedSender<ApplicationEvent>,
    pending: &mut VecDeque<ApplicationCommand>,
    generation: u64,
    input: String,
) -> ReplyOutcome {
    let update_events = events.clone();
    let reply = conversation.reply(input, |update| {
        let kind = match update {
            StreamUpdate::Text(text) => ConversationEvent::ReplyDelta(text),
            StreamUpdate::Reset => ConversationEvent::ReplyReset,
        };
        let _ = update_events.send(ApplicationEvent { generation, kind });
    });
    tokio::pin!(reply);

    loop {
        tokio::select! {
            biased;
            _ = events.closed() => return ReplyOutcome::Stopped,
            command = commands.recv() => match command {
                Some(ApplicationCommand::Clear) => return ReplyOutcome::Cleared,
                Some(command @ ApplicationCommand::Chat { .. }) => pending.push_back(command),
                None => return ReplyOutcome::Stopped,
            },
            result = &mut reply => return ReplyOutcome::Completed(result),
        }
    }
}

enum ReplyOutcome {
    Stopped,
    Cleared,
    Completed(Result<String, AppError>),
}
