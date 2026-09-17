#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Message {
    pub(super) role: Role,
    pub(super) content: String,
}

impl Message {
    pub(super) fn new(role: Role, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Role {
    System,
    User,
    Assistant,
}
