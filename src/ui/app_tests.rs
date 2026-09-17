use super::App;
use crate::application::protocol::{ApplicationCommand, ApplicationEvent, ConversationEvent};
use crate::capability::CapabilitySummary;
use crate::error::AppError;
use crate::ui::message::{Message, Role};
use tokio::sync::mpsc;

fn app() -> App {
    let (command_tx, _command_rx) = mpsc::unbounded_channel();
    let (_event_tx, event_rx) = mpsc::unbounded_channel();
    App::new("test-model".to_owned(), command_tx, event_rx)
}

#[test]
fn reply_is_added_to_the_conversation() {
    let mut app = app();
    app.busy = true;
    app.handle_application_event(ApplicationEvent {
        generation: 0,
        kind: ConversationEvent::Reply("你好".to_owned()),
    });
    assert!(!app.busy);
    assert_eq!(app.messages.last().unwrap().content, "你好");
}

#[test]
fn input_is_buffered_while_a_reply_is_in_progress() {
    let (commands, mut receiver) = mpsc::unbounded_channel();
    let (_events, events_rx) = mpsc::unbounded_channel();
    let mut app = App::new("test".to_owned(), commands, events_rx);
    app.busy = true;
    app.input = "下一条".to_owned();
    app.cursor = app.input.len();

    app.submit();

    assert_eq!(
        app.buffered_inputs.front().map(String::as_str),
        Some("下一条")
    );
    assert!(app.input.is_empty());
    assert_eq!(app.cursor, 0);
    assert!(receiver.try_recv().is_err());
    assert!(
        app.messages
            .iter()
            .all(|message| message.content != "下一条")
    );
}

#[test]
fn buffered_inputs_are_submitted_in_order_after_each_reply() {
    let (commands, mut receiver) = mpsc::unbounded_channel();
    let (_events, events_rx) = mpsc::unbounded_channel();
    let mut app = App::new("test".to_owned(), commands, events_rx);
    app.busy = true;
    for input in ["第二条", "第三条"] {
        app.input = input.to_owned();
        app.cursor = app.input.len();
        app.submit();
    }

    app.handle_application_event(ApplicationEvent {
        generation: 0,
        kind: ConversationEvent::Reply("第一条回复".to_owned()),
    });

    assert!(app.busy);
    assert_eq!(
        app.buffered_inputs.front().map(String::as_str),
        Some("第三条")
    );
    assert!(matches!(
        receiver.try_recv().unwrap(),
        ApplicationCommand::Chat { input, .. } if input == "第二条"
    ));
    assert_eq!(app.messages.last().unwrap().content, "第二条");

    app.handle_application_event(ApplicationEvent {
        generation: 0,
        kind: ConversationEvent::Reply("第二条回复".to_owned()),
    });

    assert!(app.busy);
    assert!(app.buffered_inputs.is_empty());
    assert!(matches!(
        receiver.try_recv().unwrap(),
        ApplicationCommand::Chat { input, .. } if input == "第三条"
    ));
}

#[test]
fn failed_reply_also_releases_the_next_buffered_input() {
    let (commands, mut receiver) = mpsc::unbounded_channel();
    let (_events, events_rx) = mpsc::unbounded_channel();
    let mut app = App::new("test".to_owned(), commands, events_rx);
    app.busy = true;
    app.buffered_inputs.push_back("失败后继续".to_owned());

    app.handle_application_event(ApplicationEvent {
        generation: 0,
        kind: ConversationEvent::Error(AppError::MissingFinalResponse.user_facing()),
    });

    assert!(app.busy);
    assert!(app.buffered_inputs.is_empty());
    assert!(matches!(
        receiver.try_recv().unwrap(),
        ApplicationCommand::Chat { input, .. } if input == "失败后继续"
    ));
}

#[test]
fn streamed_reply_is_updated_in_place_and_finished() {
    let mut app = app();
    app.busy = true;
    for delta in ["你", "好"] {
        app.handle_application_event(ApplicationEvent {
            generation: 0,
            kind: ConversationEvent::ReplyDelta(delta.to_owned()),
        });
    }
    assert!(app.busy);
    assert_eq!(app.messages.last().unwrap().content, "你好");

    app.handle_application_event(ApplicationEvent {
        generation: 0,
        kind: ConversationEvent::Reply("你好！".to_owned()),
    });
    assert!(!app.busy);
    assert!(app.streaming_message.is_none());
    assert_eq!(app.messages.last().unwrap().content, "你好！");
}

#[test]
fn failed_stream_keeps_partial_reply_and_marks_it_interrupted() {
    let mut app = app();
    app.busy = true;
    app.handle_application_event(ApplicationEvent {
        generation: 0,
        kind: ConversationEvent::ReplyDelta("已生成".to_owned()),
    });
    app.handle_application_event(ApplicationEvent {
        generation: 0,
        kind: ConversationEvent::Error(AppError::MissingFinalResponse.user_facing()),
    });

    assert!(!app.busy);
    assert!(app.streaming_message.is_none());
    assert_eq!(
        app.messages.last().unwrap().content,
        "已生成\n\n[请求失败：模型请求失败，请检查网络、模型配置和服务状态后重试。]"
    );
}

#[test]
fn worker_stopped_finishes_partial_reply_without_submitting_buffered_input() {
    let mut app = app();
    app.busy = true;

    app.handle_application_event(ApplicationEvent {
        generation: 0,
        kind: ConversationEvent::ReplyDelta("部分回复".to_owned()),
    });
    app.buffered_inputs.push_back("尚未发送".to_owned());
    app.finish_with_error("后台已停止");
    assert!(!app.busy);
    assert!(app.streaming_message.is_none());
    assert_eq!(
        app.messages.last().unwrap().content,
        "部分回复\n\n[后台已停止]"
    );
    assert_eq!(app.buffered_inputs.front().unwrap(), "尚未发送");
}

#[test]
fn clear_ignores_old_events_and_allows_a_new_turn() {
    let (commands, mut receiver) = mpsc::unbounded_channel();
    let (_events, events_rx) = mpsc::unbounded_channel();
    let mut app = App::new("test".to_owned(), commands, events_rx);
    app.busy = true;
    app.buffered_inputs.push_back("待清空".to_owned());
    app.clear();
    assert!(app.buffered_inputs.is_empty());
    app.input = "新消息".to_owned();
    app.submit();
    app.handle_application_event(ApplicationEvent {
        generation: 0,
        kind: ConversationEvent::Error(AppError::MissingFinalResponse.user_facing()),
    });
    assert!(app.busy);
    assert_eq!(app.messages.len(), 1);
    assert!(matches!(
        receiver.try_recv().unwrap(),
        ApplicationCommand::Clear
    ));
    assert!(matches!(
        receiver.try_recv().unwrap(),
        ApplicationCommand::Chat { generation: 1, .. }
    ));
}

#[test]
fn disconnected_worker_preserves_unsent_input() {
    let mut app = app();
    app.input = "未发送".to_owned();
    app.cursor = app.input.len();
    app.submit();
    assert_eq!(app.input, "未发送");
    assert!(!app.busy);
    assert!(
        app.messages
            .iter()
            .all(|message| message.role != Role::User)
    );
}

#[test]
fn removed_model_command_is_reported_as_unknown() {
    let mut app = app();
    app.input = "/model".to_owned();

    app.submit();

    assert_eq!(
        app.messages.last().unwrap().content,
        "未知命令或多余参数，请输入 /help 查看用法。"
    );
}

#[test]
fn skill_and_tool_commands_list_mounted_capabilities() {
    let mut app = app().with_capabilities(
        vec![CapabilitySummary {
            name: "conversation".to_owned(),
            description: "日常对话".to_owned(),
        }],
        vec![CapabilitySummary {
            name: "shell".to_owned(),
            description: "在本机执行命令".to_owned(),
        }],
    );

    app.input = "/skill".to_owned();
    app.submit();
    assert_eq!(
        app.messages.last().unwrap().content,
        "当前挂载的 Skill（1）：\n- conversation：日常对话"
    );

    app.input = "/tool".to_owned();
    app.submit();
    assert_eq!(
        app.messages.last().unwrap().content,
        "当前挂载的 Tool（1）：\n- shell：在本机执行命令"
    );
}

#[test]
fn skill_and_tool_commands_report_when_none_are_mounted() {
    let mut app = app();
    app.input = "/skill".to_owned();
    app.submit();
    assert!(
        app.messages
            .last()
            .unwrap()
            .content
            .contains("未挂载 Skill")
    );

    app.input = "/tool".to_owned();
    app.submit();
    assert!(app.messages.last().unwrap().content.contains("未挂载 Tool"));
}

#[test]
fn drawing_keeps_latest_reply_visible_across_multiline_turns() {
    use ratatui::{Terminal, backend::TestBackend};

    for (width, height) in [(80, 24), (32, 12)] {
        let mut app = app();
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        for round in 1..=5 {
            app.messages
                .push(Message::new(Role::User, format!("第 {round} 轮")));
            app.busy = true;
            let marker = format!("END{round}");
            let reply = format!("第一段中文回复需要按终端宽度换行。\n\n第二段\n\n{marker}");
            for event in [
                ConversationEvent::ReplyDelta(reply.clone()),
                ConversationEvent::Reply(reply),
            ] {
                app.handle_application_event(ApplicationEvent {
                    generation: 0,
                    kind: event,
                });
                terminal.draw(|frame| app.draw(frame)).unwrap();
                let buffer = terminal.backend().buffer();
                // 当前回复末尾必须在对话区内可见，无需下一次输入。
                let visible = (1..height - 5)
                    .map(|y| {
                        (1..width - 1)
                            .map(|x| buffer[(x, y)].symbol())
                            .collect::<String>()
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                assert!(
                    visible.contains(&marker),
                    "{width}x{height} 第 {round} 轮末尾不可见：{visible:?}"
                );
            }
        }
    }
}

#[test]
fn drawing_preserves_manual_scroll() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::{Terminal, backend::TestBackend};

    let mut app = app();
    app.messages = (0..30)
        .map(|index| Message::new(Role::User, format!("第 {index} 条消息")))
        .collect();
    let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
    terminal.draw(|frame| app.draw(frame)).unwrap();
    app.handle_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE));
    let manual_offset = app.conversation_scroll.offset;
    app.messages.push(Message::new(Role::Assistant, "新回复"));
    terminal.draw(|frame| app.draw(frame)).unwrap();
    assert_eq!(app.conversation_scroll.offset, manual_offset);
}

#[test]
fn keyboard_editing_preserves_chinese_boundaries_and_submit() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let (commands, mut receiver) = mpsc::unbounded_channel();
    let (_events, events_rx) = mpsc::unbounded_channel();
    let mut app = App::new("test".to_owned(), commands, events_rx);
    for code in [
        KeyCode::Char('你'),
        KeyCode::Char('好'),
        KeyCode::Left,
        KeyCode::Backspace,
        KeyCode::Char('您'),
        KeyCode::Delete,
        KeyCode::Char('好'),
    ] {
        app.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
        assert!(app.input.is_char_boundary(app.cursor));
    }
    assert_eq!(app.input, "您好");
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(
        matches!(receiver.try_recv().unwrap(), ApplicationCommand::Chat { input, .. } if input == "您好")
    );
    assert!(app.input.is_empty());
    assert_eq!(app.cursor, 0);
}
