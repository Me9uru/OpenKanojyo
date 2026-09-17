use anyhow::Result;
use rig_agent::{
    Agent, AgentBuilder,
    tool::{Tool, server::ToolServer},
};
use rig_core::{client::CompletionClient, providers::openai};

use crate::{
    capability::CapabilitySummary,
    config::Config,
    context::assembly::PromptContext,
    skills::{conversation::ConversationSkill, registry::SkillRegistry},
    tools::{load_skill::LoadSkillTool, shell::ShellTool},
};

pub struct AssembledAgent {
    pub agent: Agent,
    pub provider_label: String,
    pub skill_summaries: Vec<CapabilitySummary>,
    pub tool_summaries: Vec<CapabilitySummary>,
}

/// 根据启动配置装配模型、Prompt、Skill 和 Tool。
pub fn assemble_agent(config: Config) -> Result<AssembledAgent> {
    let Config {
        api_key,
        base_url,
        model,
        temperature,
        max_tokens,
        enable_thinking,
        enable_skills,
        enable_tools,
    } = config;

    let provider_label = format!("{model} @ {base_url}");
    let client = openai::CompletionsClient::builder()
        .api_key(api_key)
        .base_url(base_url)
        .build()?;
    let model = client.completion_model(model);

    let skills = if enable_skills {
        SkillRegistry::new().with_skill(ConversationSkill)
    } else {
        SkillRegistry::new()
    };
    let context = PromptContext::standard().with_skills(&skills);
    let skill_summaries = context.skill_summaries();
    let preamble = context.render_preamble();

    let mut builder = AgentBuilder::new(model)
        .name("open_kanojyo")
        .description("OpenKanojyo TUI 对话助手")
        .default_max_turns(4)
        .preamble(&preamble)
        .temperature(temperature)
        .max_tokens(max_tokens);
    if let Some(enable_thinking) = enable_thinking {
        builder = builder.additional_params(serde_json::json!({
            "chat_template_kwargs": {"enable_thinking": enable_thinking}
        }));
    }
    for document in context.render_documents() {
        builder = builder.context(&document);
    }

    let mut tool_server = ToolServer::new();
    let mut tool_summaries = Vec::new();
    if !skills.is_empty() {
        let loader = LoadSkillTool::new(skills);
        tool_summaries.push(tool_summary(&loader));
        tool_server = tool_server.tool(loader);
    }
    if enable_tools {
        let shell = ShellTool::default();
        tool_summaries.push(tool_summary(&shell));
        tool_server = tool_server.tool(shell);
    }

    Ok(AssembledAgent {
        agent: builder.tool_server_handle(tool_server.run()).build(),
        provider_label,
        skill_summaries,
        tool_summaries,
    })
}

fn tool_summary<T: Tool>(tool: &T) -> CapabilitySummary {
    CapabilitySummary {
        name: T::NAME.to_owned(),
        description: tool.description(),
    }
}
