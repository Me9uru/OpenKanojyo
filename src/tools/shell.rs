mod process;

use rig_agent::tool::{Tool, ToolContext};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::ToolInputError;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ShellArgs {
    /// 交给 `sh -c` 解释的完整命令行
    pub command: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ShellOutput {
    pub command: String,
    pub working_directory: String,
    /// 被信号终止或超时的情况下为 null。
    pub exit_code: Option<i32>,
    /// 超时终止时保留已经读到的部分输出。
    pub stdout: String,
    pub stderr: String,
    pub truncated: bool,
    pub timed_out: bool,
}

#[derive(Default)]
pub struct ShellTool;

impl Tool for ShellTool {
    const NAME: &'static str = "shell";
    type Args = ShellArgs;
    type Output = ShellOutput;
    type Error = ToolInputError;

    fn description(&self) -> String {
        format!(
            "在当前工作目录下用 `sh -c` 执行一条 shell 命令，返回退出码、标准输出和标准错误。\
             命令对用户机器有真实副作用，只在用户明确要求时执行，破坏性操作先向用户确认。\
             超过 {} 秒会被终止并返回 timed_out，同时保留已经产生的部分输出；\
             stdout 和 stderr 各自最多保留 {} 字节，超出部分丢弃并将 truncated 置为 true。\
             非零退出码是正常结果，不是工具错误。",
            process::DEFAULT_TIMEOUT.as_secs(),
            process::DEFAULT_MAX_OUTPUT_BYTES
        )
    }

    fn parameters(&self) -> serde_json::Value {
        crate::tools::schema::parameters::<ShellArgs>()
    }

    async fn call(
        &self,
        _context: &mut ToolContext,
        args: Self::Args,
    ) -> Result<Self::Output, Self::Error> {
        let command = args.command.trim();
        if command.is_empty() {
            return Err(ToolInputError("命令不能为空"));
        }
        process::run(command).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn rejects_blank_commands() {
        let error = ShellTool
            .call(
                &mut ToolContext::default(),
                ShellArgs {
                    command: "   ".to_owned(),
                },
            )
            .await
            .unwrap_err();

        assert_eq!(error.to_string(), "命令不能为空");
    }
}
