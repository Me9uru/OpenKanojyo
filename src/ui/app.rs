use std::{collections::VecDeque, io::stdout, time::Duration};

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind},
    execute,
};
use ratatui::DefaultTerminal;
use tokio::sync::mpsc;

use crate::{
    application::protocol::{ApplicationCommand, ApplicationEvent, ConversationEvent},
    capability::CapabilitySummary,
    ui::{
        message::{Message, Role},
        scroll::ScrollState,
    },
};

pub struct App {
    messages: Vec<Message>,
    input: String,
    cursor: usize,
    // 尚未交给 application 的输入；仅提交后才显示为用户消息。
    buffered_inputs: VecDeque<String>,
    busy: bool,
    streaming_message: Option<usize>,
    generation: u64,
    should_quit: bool,
    conversation_scroll: ScrollState,
    provider_label: String,
    skills: Vec<CapabilitySummary>,
    tools: Vec<CapabilitySummary>,
    commands: mpsc::UnboundedSender<ApplicationCommand>,
    events: mpsc::UnboundedReceiver<ApplicationEvent>,
}

impl App {
    pub fn new(
        provider_label: String,
        commands: mpsc::UnboundedSender<ApplicationCommand>,
        events: mpsc::UnboundedReceiver<ApplicationEvent>,
    ) -> Self {
        Self {
            messages: vec![Message::new(
                Role::Assistant,
                "你好，我是 OpenKanojyo。输入消息并按 Enter 开始对话，/help 查看命令。",
            )],
            input: String::new(),
            cursor: 0,
            buffered_inputs: VecDeque::new(),
            busy: false,
            streaming_message: None,
            generation: 0,
            should_quit: false,
            conversation_scroll: ScrollState::default(),
            provider_label,
            skills: Vec::new(),
            tools: Vec::new(),
            commands,
            events,
        }
    }

    pub fn with_capabilities(
        mut self,
        skills: Vec<CapabilitySummary>,
        tools: Vec<CapabilitySummary>,
    ) -> Self {
        self.skills = skills;
        self.tools = tools;
        self
    }

    pub fn run(mut self) -> Result<()> {
        let mut terminal = ratatui::init();
        if let Err(error) = execute!(stdout(), EnableMouseCapture) {
            ratatui::restore();
            return Err(error.into());
        }
        let result = self.event_loop(&mut terminal);
        let mouse_result = execute!(stdout(), DisableMouseCapture);
        ratatui::restore();
        result?;
        mouse_result?;
        Ok(())
    }

    fn event_loop(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        let mut dirty = true;
        while !self.should_quit {
            loop {
                match self.events.try_recv() {
                    Ok(event) => {
                        dirty |= event.generation == self.generation;
                        self.handle_application_event(event);
                    }
                    Err(mpsc::error::TryRecvError::Disconnected) if self.busy => {
                        self.finish_with_error("请求失败：后台任务已停止，请重新启动应用。");
                        dirty = true;
                        break;
                    }
                    Err(_) => break,
                }
            }

            if dirty {
                terminal.draw(|frame| self.draw(frame))?;
                dirty = false;
            }
            if event::poll(Duration::from_millis(50))? {
                match event::read()? {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        dirty |= self.handle_key(key);
                    }
                    Event::Mouse(mouse) => {
                        dirty |= self.handle_mouse(mouse);
                    }
                    Event::Resize(_, _) => dirty = true,
                    _ => {}
                }
            }
        }
        Ok(())
    }

    fn handle_application_event(&mut self, event: ApplicationEvent) {
        if event.generation != self.generation {
            return;
        }
        self.handle_conversation_event(event.kind);
    }

    fn handle_conversation_event(&mut self, event: ConversationEvent) {
        match event {
            ConversationEvent::ReplyDelta(delta) => {
                if delta.is_empty() {
                    return;
                }
                if let Some(index) = self.streaming_message {
                    if let Some(message) = self.messages.get_mut(index) {
                        message.content.push_str(&delta);
                    }
                } else {
                    self.messages.push(Message::new(Role::Assistant, delta));
                    self.streaming_message = Some(self.messages.len() - 1);
                }
                self.conversation_scroll.follow_tail = true;
            }
            ConversationEvent::ReplyReset => {
                if let Some(index) = self.streaming_message
                    && let Some(message) = self.messages.get_mut(index)
                {
                    message.content.clear();
                }
                self.conversation_scroll.follow_tail = true;
            }
            ConversationEvent::Reply(content) => {
                self.busy = false;
                if let Some(index) = self.streaming_message.take() {
                    if let Some(streaming_message) = self.messages.get_mut(index) {
                        streaming_message.content = content;
                    }
                } else {
                    self.messages.push(Message::new(Role::Assistant, content));
                }
                self.conversation_scroll.follow_tail = true;
                self.submit_buffered_input();
            }
            ConversationEvent::Error(error) => {
                self.finish_with_error(error.message());
                self.submit_buffered_input();
            }
        }
    }

    fn finish_with_error(&mut self, message: &str) {
        self.busy = false;
        if let Some(index) = self.streaming_message.take() {
            if let Some(streaming) = self.messages.get_mut(index) {
                if streaming.content.is_empty() {
                    streaming.content = message.to_owned();
                } else {
                    streaming.content.push_str("\n\n[");
                    streaming.content.push_str(message);
                    streaming.content.push(']');
                }
            }
        } else {
            self.messages.push(Message::new(Role::Assistant, message));
        }
        self.conversation_scroll.follow_tail = true;
    }

    fn submit(&mut self) {
        let input = self.input.trim().to_owned();
        if input.is_empty() {
            return;
        }
        if input.starts_with('/') {
            self.input.clear();
            self.cursor = 0;
            self.execute_command(crate::ui::interaction::commands::parse(&input));
            return;
        }
        if self.busy {
            self.buffered_inputs.push_back(input);
            self.input.clear();
            self.cursor = 0;
            return;
        }
        if self.send_chat(input) {
            self.input.clear();
            self.cursor = 0;
        }
    }

    fn send_chat(&mut self, input: String) -> bool {
        if self
            .commands
            .send(ApplicationCommand::Chat {
                generation: self.generation,
                input: input.clone(),
            })
            .is_ok()
        {
            self.messages.push(Message::new(Role::User, input));
            self.busy = true;
            self.conversation_scroll.follow_tail = true;
            true
        } else {
            self.messages.push(Message::new(
                Role::Assistant,
                "后台任务已停止，请退出并重新启动应用。",
            ));
            false
        }
    }

    fn submit_buffered_input(&mut self) {
        let Some(input) = self.buffered_inputs.pop_front() else {
            return;
        };
        if !self.send_chat(input.clone()) {
            self.buffered_inputs.push_front(input);
        }
    }

    fn execute_command(&mut self, command: crate::ui::interaction::commands::Command) {
        use crate::ui::interaction::commands::{Command, HELP, capability_list};
        match command {
            Command::Clear => {
                self.clear();
                self.notice("已新开对话。".to_owned());
            }
            Command::Skills => self.notice(capability_list("Skill", &self.skills,
                "当前未挂载 Skill。可通过 AGENT_ENABLE_SKILLS=true 启用内置 Skill。")),
            Command::Tools => self.notice(capability_list("Tool", &self.tools,
                "当前未挂载 Tool。可通过 AGENT_ENABLE_SKILLS=true 启用 Skill 加载器，或通过 AGENT_ENABLE_TOOLS=true 启用 shell。")),
            Command::Help => self.notice(HELP.to_owned()),
            Command::Quit => self.should_quit = true,
            Command::Unknown => self.notice("未知命令或多余参数，请输入 /help 查看用法。".to_owned()),
        }
    }

    fn notice(&mut self, content: String) {
        self.messages.push(Message::new(Role::System, content));
        self.conversation_scroll.follow_tail = true;
    }

    fn clear(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.busy = false;
        self.streaming_message = None;
        self.buffered_inputs.clear();
        self.messages.clear();
        let _ = self.commands.send(ApplicationCommand::Clear);
        self.conversation_scroll = ScrollState::default();
    }

    fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> bool {
        self.apply_action(crate::ui::interaction::input::key_action(key))
    }

    fn handle_mouse(&mut self, mouse: crossterm::event::MouseEvent) -> bool {
        self.apply_action(crate::ui::interaction::input::mouse_action(mouse))
    }

    fn apply_action(&mut self, action: Option<crate::ui::interaction::input::Action>) -> bool {
        use crate::ui::interaction::input::Action;
        match action {
            Some(Action::Quit) => self.should_quit = true,
            Some(Action::Submit) => self.submit(),
            Some(Action::Clear) => self.clear(),
            Some(Action::Insert(character)) => {
                self.input.insert(self.cursor, character);
                self.cursor += character.len_utf8();
            }
            Some(Action::Backspace) => {
                if let Some(previous) = previous_boundary(&self.input, self.cursor) {
                    self.input.drain(previous..self.cursor);
                    self.cursor = previous;
                }
            }
            Some(Action::Delete) if self.cursor < self.input.len() => {
                let next =
                    self.cursor + self.input[self.cursor..].chars().next().unwrap().len_utf8();
                self.input.drain(self.cursor..next);
            }
            Some(Action::Left) => {
                if let Some(previous) = previous_boundary(&self.input, self.cursor) {
                    self.cursor = previous;
                }
            }
            Some(Action::Right) if self.cursor < self.input.len() => {
                self.cursor += self.input[self.cursor..].chars().next().unwrap().len_utf8();
            }
            Some(Action::Home) => self.cursor = 0,
            Some(Action::End) => self.cursor = self.input.len(),
            Some(Action::FollowTail) => self.conversation_scroll.follow_tail = true,
            Some(Action::ScrollUp(amount)) => self.conversation_scroll.up(amount),
            Some(Action::ScrollDown(amount)) => self.conversation_scroll.down(amount),
            _ => return false,
        }
        true
    }

    fn draw(&mut self, frame: &mut ratatui::Frame) {
        use crate::ui::view::render::{View, draw};
        let view = View {
            messages: &self.messages,
            input: &self.input,
            cursor: self.cursor,
            busy: self.busy,
            streaming: self.streaming_message.is_some(),
            buffered_count: self.buffered_inputs.len(),
            provider_label: &self.provider_label,
        };
        draw(&view, &mut self.conversation_scroll, frame);
    }
}

fn previous_boundary(value: &str, cursor: usize) -> Option<usize> {
    value[..cursor]
        .char_indices()
        .next_back()
        .map(|(index, _)| index)
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
