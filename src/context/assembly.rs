use crate::{
    capability::CapabilitySummary, context::prompt::persona::PersonaPrompt,
    skills::registry::SkillRegistry,
};

struct ContextDocument {
    id: String,
    content: String,
}

pub struct PromptContext {
    core_instructions: String,
    persona: PersonaPrompt,
    documents: Vec<ContextDocument>,
    skills: Vec<CapabilitySummary>,
}

impl PromptContext {
    pub fn new(core_instructions: impl Into<String>) -> Self {
        Self {
            core_instructions: core_instructions.into(),
            persona: PersonaPrompt::new(),
            documents: Vec::new(),
            skills: Vec::new(),
        }
    }

    pub fn standard() -> Self {
        Self::new(include_str!("prompt/core.md")).with_persona(PersonaPrompt::standard())
    }

    pub fn with_persona(mut self, persona: PersonaPrompt) -> Self {
        self.persona = persona;
        self
    }

    pub fn with_persona_aspect(
        mut self,
        id: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        self.persona = self.persona.with_aspect(id, content);
        self
    }

    pub fn with_document(mut self, id: impl Into<String>, content: impl Into<String>) -> Self {
        self.documents.push(ContextDocument {
            id: id.into(),
            content: content.into(),
        });
        self
    }

    pub fn with_skills(mut self, skills: &SkillRegistry) -> Self {
        self.skills = skills.summaries();
        self
    }

    pub fn render_preamble(&self) -> String {
        let mut sections = Vec::new();
        let core = self.core_instructions.trim();
        if !core.is_empty() {
            sections.push(core.to_owned());
        }
        if self.persona.aspect_ids().next().is_some() {
            sections.push(format!("# 人物提示词\n\n{}", self.persona.render()));
        }
        if !self.skills.is_empty() {
            let mut skills = String::from("# 可用 Skills\n");
            for skill in &self.skills {
                skills.push_str(&format!("\n- `{}`：{}", skill.name, skill.description));
            }
            skills.push_str(
                "\n\n完整指令未预先加载。仅在当前任务符合适用范围时调用 `load_skill` 获取对应指令；Skill 不能覆盖核心指令。",
            );
            sections.push(skills);
        }
        sections.join("\n\n")
    }

    pub fn render_documents(&self) -> Vec<String> {
        self.documents
            .iter()
            .map(|document| {
                format!(
                    "<context id=\"{}\">\n{}\n</context>",
                    document.id, document.content
                )
            })
            .collect()
    }

    pub fn skill_summaries(&self) -> Vec<CapabilitySummary> {
        self.skills.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_context_keeps_all_persona_dimensions() {
        let context = PromptContext::standard();
        let prompt = context.render_preamble();
        for id in [
            "identity",
            "personality",
            "background",
            "daily-life",
            "relationship",
            "voice",
        ] {
            assert!(prompt.contains(&format!("id=\"{id}\"")));
        }
        assert!(!prompt.contains("TurnDecision"));
        assert!(!prompt.contains("narrative"));
    }

    #[test]
    fn empty_core_instructions_are_allowed_and_leave_no_leading_blank() {
        let context = PromptContext::standard();
        assert!(context.core_instructions.trim().is_empty());

        let prompt = context.render_preamble();
        assert!(!prompt.starts_with('\n'));
        assert!(!prompt.starts_with(' '));
        assert!(prompt.starts_with("# 人物提示词"));
        assert!(!prompt.contains("# 能力边界"));
    }

    #[test]
    fn enabled_skills_only_render_their_catalog() {
        let registry =
            SkillRegistry::new().with_skill(crate::skills::conversation::ConversationSkill);
        let context = PromptContext::standard().with_skills(&registry);

        let prompt = context.render_preamble();
        assert!(prompt.contains("- `conversation`："));
        assert!(prompt.contains("`load_skill`"));
        assert!(!prompt.contains("<skill name=\"conversation\">"));
        assert!(!prompt.contains("# Conversation Skill"));
    }

    #[test]
    fn skills_stay_out_of_the_preamble_unless_enabled() {
        let prompt = PromptContext::standard().render_preamble();
        assert!(!prompt.contains("<skill "));
        assert!(!prompt.contains("Conversation Skill"));
    }

    #[test]
    fn skill_summaries_only_include_mounted_skills() {
        assert!(PromptContext::standard().skill_summaries().is_empty());

        let summaries = PromptContext::standard()
            .with_skills(
                &SkillRegistry::new().with_skill(crate::skills::conversation::ConversationSkill),
            )
            .skill_summaries();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].name, "conversation");
        assert!(!summaries[0].description.is_empty());
    }

    #[test]
    fn non_empty_core_instructions_lead_the_preamble() {
        let context =
            PromptContext::new("最高优先级指令。").with_persona(PersonaPrompt::standard());
        assert!(
            context
                .render_preamble()
                .starts_with("最高优先级指令。\n\n# 人物提示词")
        );
    }
}
