use crate::error::UserFacingError;

pub enum ApplicationCommand {
    Chat { generation: u64, input: String },
    Clear,
}

pub struct ApplicationEvent {
    pub generation: u64,
    pub kind: ConversationEvent,
}

pub enum ConversationEvent {
    ReplyDelta(String),
    ReplyReset,
    Reply(String),
    Error(UserFacingError),
}
