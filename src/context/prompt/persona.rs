#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PersonaAspect {
    id: String,
    content: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PersonaPrompt {
    aspects: Vec<PersonaAspect>,
}

impl PersonaPrompt {
    pub fn new() -> Self {
        Self::default()
    }

    /// 目前只保留一套人物设定，六个维度按顺序装配。
    pub fn standard() -> Self {
        Self::new()
            .with_aspect("identity", include_str!("personas/identity.md"))
            .with_aspect("personality", include_str!("personas/personality.md"))
            .with_aspect("background", include_str!("personas/background.md"))
            .with_aspect("daily-life", include_str!("personas/daily-life.md"))
            .with_aspect("relationship", include_str!("personas/relationship.md"))
            .with_aspect("voice", include_str!("personas/voice.md"))
    }

    pub fn with_aspect(mut self, id: impl Into<String>, content: impl Into<String>) -> Self {
        self.aspects.push(PersonaAspect {
            id: id.into(),
            content: content.into(),
        });
        self
    }

    pub fn render(&self) -> String {
        self.aspects
            .iter()
            .map(|aspect| {
                format!(
                    "<persona-aspect id=\"{}\">\n{}\n</persona-aspect>",
                    aspect.id,
                    aspect.content.trim()
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    pub fn aspect_ids(&self) -> impl Iterator<Item = &str> {
        self.aspects.iter().map(|aspect| aspect.id.as_str())
    }
}
