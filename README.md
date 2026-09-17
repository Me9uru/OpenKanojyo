# OpenKanojyo

OpenKanojyo 是一个使用 Rust、Rig 和 Ratatui 构建的终端 LLM 对话应用。用户消息通过
Rig 的流式 Chat 接口发送给模型，文本增量会实时显示；应用保留人物提示词、可选 Skill 和原生 Tool 调用，但不再
生成小说剧本、时间轴或延迟投递决策。

## 运行

复制配置模板并填写 API Key：

```bash
cp .env.example .env
cargo run
```

支持 OpenAI 和兼容 OpenAI Chat Completions API 的服务。环境变量如下：

| 变量 | 必填 | 默认值 | 用途 |
| --- | --- | --- | --- |
| `OPENAI_API_KEY` | 是 | — | API 凭据 |
| `OPENAI_MODEL` | 是 | — | 对话模型名 |
| `OPENAI_BASE_URL` | 否 | `https://api.openai.com/v1` | API 地址 |
| `AGENT_TEMPERATURE` | 否 | `0.7` | 采样温度，范围 0 到 2 |
| `AGENT_MAX_TOKENS` | 否 | `1024` | 单次回复的最大 token 数，必须大于 0 |
| `AGENT_ENABLE_THINKING` | 否 | 不设置 | 向支持它的兼容服务传递 `chat_template_kwargs.enable_thinking` |
| `AGENT_ENABLE_SKILLS` | 否 | `false` | 公布内置 Skill 摘要，并注册 `load_skill` 按需加载完整指令 |
| `AGENT_ENABLE_TOOLS` | 否 | `false` | 注册 shell 工具，模型可在本机执行 shell 命令 |

对话上下文及其人物提示词位于 `src/context/`，Skill 位于 `src/skills/`，都不从环境变量读取。
`src/context/prompt/core.md` 是最高优先级指令的预留位置，当前为空且在提示词中不占位。
人物设定在 `src/context/prompt/personas/` 下，由
identity、personality、background、daily-life、relationship、voice 六个 Markdown 维度按
顺序组成，只保留普拉娜（Plana）这一套，用第二人称撰写，设定是老师（用户）创造的 AI、这台
机器的操作系统管理者，不承认 AI 身份那类设定已经移除。`.env` 已加入 `.gitignore`，
`.env.example` 只保存无效示例凭据。

## Skill 与 Tool

Skill 与具备副作用的 Tool 独立启用。启用 Skill 时，preamble 只包含名称和说明；模型确认任务适用后，
通过只读的 `load_skill` Tool 获取完整指令，实现渐进式披露。Tool 通过 Rig 原生工具调用协议执行，
执行完后由同一个模型继续生成普通聊天文本，不使用结构化小说回合协议。

| Tool | 用途与边界 |
| --- | --- |
| `load_skill` | 按名称读取已启用 Skill 的完整指令；随 `AGENT_ENABLE_SKILLS=true` 注册 |
| `shell` | 用 `sh -c` 在当前工作目录执行一条命令，返回退出码、stdout 和 stderr |

`shell` 工具**对用户机器有真实副作用**，因此默认关闭，只在 `AGENT_ENABLE_TOOLS=true`
时注册。它继承父进程的工作目录和环境，命令超过 30 秒会被终止并返回 `timed_out`，
同时保留已经产生的部分输出；stdout 与 stderr 各自最多保留 8000 字节、超出部分丢弃并置 `truncated`；
非零退出码作为正常结果返回，不算工具错误。取消对话（`Ctrl+L`）会终止正在执行的命令；
回复期间发送的新消息只进入输入缓冲，不会中断命令。kill 只作用于直接子进程 `sh`，它自己派生的进程可能继续存活。
该工具依赖 POSIX `sh`，不适用于没有 `sh` 的平台。

## 命令与快捷键

- `Enter`：发送消息；模型正在回复时会按输入顺序暂存，当前回复结束后自动发送
- `PageUp` / `PageDown` 或鼠标滚轮：滚动对话
- `Ctrl+L` 或 `/clear`：取消当前生成并清空会话
- `/skill`：查看当前挂载的 Skill 及其说明
- `/tool`：查看当前挂载的 Tool 及其说明
- `/help`：显示命令帮助
- `/quit`、`Esc` 或 `Ctrl+C`：退出

## 结构

```text
src/
├── main.rs                 # 配置组件并启动应用
├── lib.rs                  # 对可执行程序暴露最小组装 API
├── agent.rs                # 模型、Prompt、Skill 与 Tool 的统一装配入口
├── capability.rs           # Skill 与 Tool 共用的能力摘要
├── config.rs               # dotenvy 配置读取与类型转换
├── error.rs                # 运行期错误分类、脱敏与用户文案
├── conversation.rs         # Rig 流式对话与进程内 Chat 历史
├── application.rs          # application 子模块声明
├── application/            # 应用协议与异步对话 worker
├── context.rs              # context 子模块声明
├── context/                # Prompt Context 装配与六维人物提示词
├── skills.rs               # skills 子模块声明
├── skills/                 # Skill 契约、内置 Skill 与其指令
├── tools.rs                # Tool 子模块声明
├── tools/                  # 各个 Rig Tool 的具体实现
├── ui.rs                   # UI 子模块声明
└── ui/                     # Ratatui 状态、输入和渲染
```

`main.rs` 只读取配置、启动 application worker 和 UI。顶层 `agent.rs` 统一消费配置，创建模型并装配
Prompt Context、Skill 与 Tool；`context/assembly.rs` 只负责生成 preamble 和附加上下文，`tools/`
中的模块只负责各个 Tool 的实现。

UI 只通过一组 `ApplicationCommand` / `ApplicationEvent` 与 application worker 通信。模型请求在
Tokio 后台任务中异步执行，不阻塞 TUI 事件循环；worker 停止后通道断开，UI 会在发送或等待回复时给出
明确提示。Conversation 只负责 Rig 对话和历史，用户、系统及助手消息的展示模型归 UI 所有。
Rig 只会在模型成功回复后提交该轮历史，失败或取消不会污染后续上下文；模型回复期间提交的新消息会在
UI 中按顺序缓冲，并在每轮请求结束后自动发送下一条。`Ctrl+L` 会递增会话代次、清空输入缓冲，旧请求的
结果不会回填新会话。
`ui/app.rs` 独占 UI 状态及提交、清空、回复完成等状态转换，字段不对兄弟模块开放。
`ui/interaction/` 只解释按键、鼠标和斜杠命令；`ui/view/` 接收只读展示数据与独立的可变滚动状态，
不依赖 `App` 或应用通信通道。初始显示、有效交互、当前会话事件和窗口尺寸变化时触发重绘，空闲轮询不重复排版历史。
UI 缓冲保存尚未提交的输入；worker 的队列用于支持公开应用接口的其他调用者连续提交请求，当前 TUI 每轮只提交一条。
此次职责整理复用已有 Crossterm、Ratatui 和 Tokio，不新增依赖或环境配置。

运行期错误在 `error.rs` 中使用 Thiserror 统一分类，application 边界将带 source 的内部错误转换为
脱敏中文文案后再传给 UI；清空操作通过会话代次隔离旧事件，不额外定义取消错误。启动组装阶段继续使用 Anyhow
传播上下文，Tool 保留所属模块的参数错误。

对话直接复用 Rig 0.42 `Agent::stream_chat`，工具通过 Rig 的 `ToolServer` 统一注册并挂到
`AgentBuilder`，由 Rig 负责解析服务端流、
续接 Tool 调用并产生最终会话历史；本地只显式声明依赖树中已有的 `futures-util`，
以最小 `std` feature 轮询文本增量，不自行解析 SSE。人物提示词装配使用项目内的轻量结构；
Skill 渐进式披露复用同一套 Tool 调用机制及原有 Schemars/Serde，不引入额外依赖。`shell` 工具使用
Tokio 1 的 `process` 与 `io-util`
feature 执行子进程，不引入额外进程管理 crate；这两个 feature 只依赖已在依赖树中的
`bytes` 与 `signal-hook-registry`。相比自行维护剧情账本和结构化回合协议，
Rig 已经提供了所需的多轮消息历史、
原生工具续接与失败后不提交语义。
