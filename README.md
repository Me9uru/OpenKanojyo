# OpenKanojyo

一个基于 [Rig](https://github.com/0xPlaygrounds/rig) Agent 范式的 Rust 对话框架，
使用 `ratatui` 提供终端界面，并通过 `dotenvy` 从 `.env` 加载配置。

## 运行

复制配置模板并填写 API Key：

```bash
cp .env.example .env
```

`.env` 中包含：

```bash
OPENAI_API_KEY=your-api-key
OPENAI_MODEL=gpt-4.1-mini
OPENAI_BASE_URL=https://api.openai.com/v1
AGENT_PREAMBLE=你是一个友善、准确、简洁的中文 AI 助手。
AGENT_TEMPERATURE=0.7
```

然后运行：

```bash
cargo run
```

`.env` 已加入 `.gitignore`，不会被提交；`.env.example` 只保存配置模板。

## 快捷键

- `Enter`：发送消息
- `PageUp` / `PageDown`：滚动对话
- `Ctrl+L`：清空会话
- `Esc` / `Ctrl+C`：退出

## 结构

- `agent.rs`：Rig Agent 包装和 Rig Chat 会话历史
- `ui.rs`：TUI 状态、渲染、输入处理和异步 Agent worker
- `config.rs`：dotenvy 配置加载与校验

后续可以直接通过 Rig 的 `AgentBuilder::tool` 添加工具，或使用 `Agent::into_tool`
构建多 Agent 编排。
