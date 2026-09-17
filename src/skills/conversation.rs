use crate::skills::contract::Skill;

/// 内置的日常对话 Skill，指令内容在 `conversation.md`。
pub struct ConversationSkill;

impl Skill for ConversationSkill {
    fn name(&self) -> &str {
        "conversation"
    }

    fn description(&self) -> &str {
        "负责清晰、友善且有上下文意识的日常对话"
    }

    fn instructions(&self) -> &str {
        include_str!("conversation.md")
    }
}
