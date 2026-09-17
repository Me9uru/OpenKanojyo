use crate::capability::CapabilitySummary;

use super::contract::Skill;

#[derive(Default)]
pub struct SkillRegistry {
    skills: Vec<Box<dyn Skill>>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_skill<S>(mut self, skill: S) -> Self
    where
        S: Skill + 'static,
    {
        self.skills.push(Box::new(skill));
        self
    }

    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }

    pub fn summaries(&self) -> Vec<CapabilitySummary> {
        self.skills
            .iter()
            .map(|skill| CapabilitySummary {
                name: skill.name().to_owned(),
                description: skill.description().to_owned(),
            })
            .collect()
    }

    pub(crate) fn get(&self, name: &str) -> Option<&dyn Skill> {
        self.skills
            .iter()
            .find(|skill| skill.name() == name)
            .map(AsRef::as_ref)
    }
}
