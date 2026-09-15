use std::env;

use anyhow::{Context, Result, bail};

pub struct Config {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
    pub preamble: String,
    pub temperature: f64,
}

impl Config {
    pub fn load() -> Result<Self> {
        dotenvy::dotenv().context("无法读取 .env，请先复制 .env.example 并填写配置")?;

        let temperature = env::var("AGENT_TEMPERATURE")
            .unwrap_or_else(|_| "0.7".to_owned())
            .parse()
            .context("AGENT_TEMPERATURE 必须是数字")?;
        if !(0.0..=2.0).contains(&temperature) {
            bail!("AGENT_TEMPERATURE 必须在 0 到 2 之间");
        }

        Ok(Self {
            api_key: required("OPENAI_API_KEY")?,
            base_url: env::var("OPENAI_BASE_URL")
                .unwrap_or_else(|_| "https://api.openai.com/v1".to_owned()),
            model: required("OPENAI_MODEL")?,
            preamble: env::var("AGENT_PREAMBLE").unwrap_or_else(|_| {
                "你是 OpenKanojyo，一个友善、准确、简洁的中文 AI 助手。".to_owned()
            }),
            temperature,
        })
    }
}

fn required(name: &str) -> Result<String> {
    let value = env::var(name).with_context(|| format!(".env 中缺少 {name}"))?;
    if value.trim().is_empty() {
        bail!(".env 中的 {name} 不能为空");
    }
    Ok(value)
}
