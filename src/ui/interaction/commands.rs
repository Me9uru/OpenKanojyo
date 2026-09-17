use crate::capability::CapabilitySummary;

pub(in crate::ui) enum Command {
    Clear,
    Skills,
    Tools,
    Help,
    Quit,
    Unknown,
}

pub(in crate::ui) fn parse(command: &str) -> Command {
    match command {
        "/clear" => Command::Clear,
        "/skill" => Command::Skills,
        "/tool" => Command::Tools,
        "/help" => Command::Help,
        "/quit" => Command::Quit,
        _ => Command::Unknown,
    }
}

pub(in crate::ui) const HELP: &str = concat!(
    "/clear：新开对话、取消当前生成并清除对话历史\n",
    "/skill：查看当前挂载的 Skill\n",
    "/tool：查看当前挂载的 Tool\n",
    "/help：显示命令帮助\n",
    "/quit：退出应用",
);

pub(in crate::ui) fn capability_list(
    name: &str,
    items: &[CapabilitySummary],
    empty: &str,
) -> String {
    if items.is_empty() {
        return empty.to_owned();
    }
    format!(
        "当前挂载的 {name}（{}）：\n{}",
        items.len(),
        items
            .iter()
            .map(|item| format!("- {}：{}", item.name, item.description))
            .collect::<Vec<_>>()
            .join("\n")
    )
}
