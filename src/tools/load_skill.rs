use rig_agent::tool::{Tool, ToolContext};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{error::ToolInputError, skills::registry::SkillRegistry};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LoadSkillArgs {
    /// 提示词中列出的 Skill 名称
    pub name: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct LoadSkillOutput {
    pub name: String,
    pub instructions: String,
}

pub struct LoadSkillTool {
    skills: SkillRegistry,
}

impl LoadSkillTool {
    pub fn new(skills: SkillRegistry) -> Self {
        Self { skills }
    }
}

impl Tool for LoadSkillTool {
    const NAME: &'static str = "load_skill";
    type Args = LoadSkillArgs;
    type Output = LoadSkillOutput;
    type Error = ToolInputError;

    fn description(&self) -> String {
        "按名称加载一个已启用 Skill 的完整指令。仅在当前任务符合提示词所列 Skill 的适用范围时调用。"
            .to_owned()
    }

    fn parameters(&self) -> serde_json::Value {
        crate::tools::schema::parameters::<LoadSkillArgs>()
    }

    async fn call(
        &self,
        _context: &mut ToolContext,
        args: Self::Args,
    ) -> Result<Self::Output, Self::Error> {
        let skill = self
            .skills
            .get(&args.name)
            .ok_or(ToolInputError("未找到该 Skill，请使用提示词中列出的名称"))?;
        Ok(LoadSkillOutput {
            name: skill.name().to_owned(),
            instructions: skill.instructions().trim().to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::contract::Skill;

    struct TestSkill;

    impl Skill for TestSkill {
        fn name(&self) -> &str {
            "test"
        }

        fn description(&self) -> &str {
            "测试 Skill"
        }

        fn instructions(&self) -> &str {
            "  完整指令。  "
        }
    }

    fn tool() -> LoadSkillTool {
        LoadSkillTool::new(SkillRegistry::new().with_skill(TestSkill))
    }

    #[tokio::test]
    async fn loads_enabled_skill_instructions_by_name() {
        let output = tool()
            .call(
                &mut ToolContext::default(),
                LoadSkillArgs {
                    name: "test".to_owned(),
                },
            )
            .await
            .unwrap();

        assert_eq!(
            output,
            LoadSkillOutput {
                name: "test".to_owned(),
                instructions: "完整指令。".to_owned(),
            }
        );
    }

    #[tokio::test]
    async fn rejects_unknown_skill_names() {
        let error = tool()
            .call(
                &mut ToolContext::default(),
                LoadSkillArgs {
                    name: "missing".to_owned(),
                },
            )
            .await
            .unwrap_err();

        assert_eq!(
            error.to_string(),
            "未找到该 Skill，请使用提示词中列出的名称"
        );
    }
}
