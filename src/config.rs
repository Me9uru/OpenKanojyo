use std::env;

use anyhow::Result;

pub struct Config {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
    pub temperature: f64,
    pub max_tokens: u64,
    pub enable_thinking: Option<bool>,
    pub enable_skills: bool,
    pub enable_tools: bool,
}

impl Config {
    pub fn load() -> Result<Self> {
        let _ = dotenvy::dotenv();

        let temperature = env::var("AGENT_TEMPERATURE")
            .unwrap_or_else(|_| "0.7".to_owned())
            .parse::<f64>()?;
        let max_tokens = env::var("AGENT_MAX_TOKENS")
            .unwrap_or_else(|_| "1024".to_owned())
            .parse::<u64>()?;
        let enable_thinking = env::var("AGENT_ENABLE_THINKING")
            .ok()
            .map(|value| value.parse::<bool>())
            .transpose()?;
        let enable_skills = env::var("AGENT_ENABLE_SKILLS")
            .unwrap_or_else(|_| "false".to_owned())
            .parse::<bool>()?;
        let enable_tools = env::var("AGENT_ENABLE_TOOLS")
            .unwrap_or_else(|_| "false".to_owned())
            .parse::<bool>()?;

        Ok(Self {
            api_key: env::var("OPENAI_API_KEY")?,
            base_url: env::var("OPENAI_BASE_URL")
                .unwrap_or_else(|_| "https://api.openai.com/v1".to_owned()),
            model: env::var("OPENAI_MODEL")?,
            temperature,
            max_tokens,
            enable_thinking,
            enable_skills,
            enable_tools,
        })
    }
}
