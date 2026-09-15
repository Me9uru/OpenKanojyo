use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Direction, Layout, Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use tokio::sync::mpsc;
use unicode_width::UnicodeWidthStr;

use crate::agent::{Agent, Message, Role};

pub enum AgentCommand {
    Chat(String),
    Clear,
}

pub enum AgentEvent {
    Reply(Message),
    Error(String),
}

pub async fn agent_worker(
    mut agent: Agent,
    mut commands: mpsc::UnboundedReceiver<AgentCommand>,
    events: mpsc::UnboundedSender<AgentEvent>,
) {
    while let Some(command) = commands.recv().await {
        match command {
            AgentCommand::Chat(input) => {
                let event = match agent.reply(input).await {
                    Ok(message) => AgentEvent::Reply(message),
                    Err(error) => AgentEvent::Error(format!("{error:#}")),
                };
                if events.send(event).is_err() {
                    break;
                }
            }
            AgentCommand::Clear => agent.clear(),
        }
    }
}

pub struct App {
    messages: Vec<Message>,
    input: String,
    cursor: usize,
    busy: bool,
    should_quit: bool,
    scroll: u16,
    follow_tail: bool,
    provider_label: String,
    commands: mpsc::UnboundedSender<AgentCommand>,
    events: mpsc::UnboundedReceiver<AgentEvent>,
}

impl App {
    pub fn new(
        provider_label: String,
        commands: mpsc::UnboundedSender<AgentCommand>,
        events: mpsc::UnboundedReceiver<AgentEvent>,
    ) -> Self {
        Self {
            messages: vec![Message::new(
                Role::Assistant,
                "你好，我是 OpenKanojyo。输入消息并按 Enter 开始对话。",
            )],
            input: String::new(),
            cursor: 0,
            busy: false,
            should_quit: false,
            scroll: 0,
            follow_tail: true,
            provider_label,
            commands,
            events,
        }
    }

    pub fn run(mut self) -> Result<()> {
        let mut terminal = ratatui::init();
        let result = self.event_loop(&mut terminal);
        ratatui::restore();
        result
    }

    fn event_loop(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        while !self.should_quit {
            while let Ok(event) = self.events.try_recv() {
                self.handle_agent_event(event);
            }

            terminal.draw(|frame| self.draw(frame))?;
            if event::poll(Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        self.handle_key(key);
                    }
                }
            }
        }
        Ok(())
    }

    fn handle_agent_event(&mut self, event: AgentEvent) {
        self.busy = false;
        match event {
            AgentEvent::Reply(message) => self.messages.push(message),
            AgentEvent::Error(error) => self
                .messages
                .push(Message::new(Role::Assistant, format!("请求失败：{error}"))),
        }
        self.follow_tail = true;
    }

    fn handle_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return;
        }

        match key.code {
            KeyCode::Esc => self.should_quit = true,
            KeyCode::Enter if !self.busy => self.submit(),
            KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL) => self.clear(),
            KeyCode::Char(character)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                self.input.insert(self.cursor, character);
                self.cursor += character.len_utf8();
            }
            KeyCode::Backspace => {
                if let Some(previous) = previous_boundary(&self.input, self.cursor) {
                    self.input.drain(previous..self.cursor);
                    self.cursor = previous;
                }
            }
            KeyCode::Delete if self.cursor < self.input.len() => {
                let next =
                    self.cursor + self.input[self.cursor..].chars().next().unwrap().len_utf8();
                self.input.drain(self.cursor..next);
            }
            KeyCode::Left => {
                if let Some(previous) = previous_boundary(&self.input, self.cursor) {
                    self.cursor = previous;
                }
            }
            KeyCode::Right if self.cursor < self.input.len() => {
                self.cursor += self.input[self.cursor..].chars().next().unwrap().len_utf8();
            }
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.input.len(),
            KeyCode::PageUp => {
                self.scroll = self.scroll.saturating_sub(5);
                self.follow_tail = false;
            }
            KeyCode::PageDown => {
                self.scroll = self.scroll.saturating_add(5);
                self.follow_tail = false;
            }
            _ => {}
        }
    }

    fn submit(&mut self) {
        let input = self.input.trim().to_owned();
        if input.is_empty() {
            return;
        }

        self.messages.push(Message::new(Role::User, input.clone()));
        if self.commands.send(AgentCommand::Chat(input)).is_ok() {
            self.busy = true;
            self.input.clear();
            self.cursor = 0;
            self.follow_tail = true;
        }
    }

    fn clear(&mut self) {
        self.messages.clear();
        let _ = self.commands.send(AgentCommand::Clear);
        self.scroll = 0;
        self.follow_tail = true;
    }

    fn draw(&mut self, frame: &mut Frame) {
        let areas = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(3),
                Constraint::Length(1),
                Constraint::Length(3),
            ])
            .split(frame.area());

        self.draw_history(frame, areas[0]);
        self.draw_status(frame, areas[1]);
        self.draw_input(frame, areas[2]);
    }

    fn draw_history(&mut self, frame: &mut Frame, area: Rect) {
        let text = conversation_text(&self.messages);
        let block = Block::default()
            .title(" OpenKanojyo / Agent ")
            .borders(Borders::ALL);
        let inner_height = area.height.saturating_sub(2) as usize;
        let width = area.width.saturating_sub(2);
        let line_count = conversation_height(&self.messages, width as usize);
        let max_scroll = line_count
            .saturating_sub(inner_height)
            .min(u16::MAX as usize) as u16;
        if self.follow_tail {
            self.scroll = max_scroll;
        } else {
            self.scroll = self.scroll.min(max_scroll);
        }

        let history = Paragraph::new(text)
            .block(block)
            .wrap(Wrap { trim: false })
            .scroll((self.scroll, 0));
        frame.render_widget(history, area);
    }

    fn draw_status(&self, frame: &mut Frame, area: Rect) {
        let state = if self.busy { "思考中…" } else { "就绪" };
        let status = Line::from(vec![
            Span::styled(format!(" {state} "), Style::default().fg(Color::Cyan)),
            Span::raw(format!(
                "│ {} │ Enter 发送  Ctrl+L 清空  Esc 退出",
                self.provider_label
            )),
        ]);
        frame.render_widget(Paragraph::new(status), area);
    }

    fn draw_input(&self, frame: &mut Frame, area: Rect) {
        let inner_width = area.width.saturating_sub(2) as usize;
        let start = visible_input_start(&self.input, self.cursor, inner_width);
        let input = Paragraph::new(&self.input[start..])
            .block(Block::default().title(" 消息 ").borders(Borders::ALL));
        frame.render_widget(input, area);

        let cursor_width = UnicodeWidthStr::width(&self.input[start..self.cursor]) as u16;
        frame.set_cursor_position(Position::new(
            area.x.saturating_add(1).saturating_add(cursor_width),
            area.y.saturating_add(1),
        ));
    }
}

fn conversation_text(messages: &[Message]) -> Text<'static> {
    let mut lines = Vec::new();
    for (index, message) in messages.iter().enumerate() {
        let (name, color) = match message.role {
            Role::User => ("You", Color::Green),
            Role::Assistant => ("Agent", Color::Cyan),
        };
        lines.push(Line::from(Span::styled(
            format!("{name}:"),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        )));
        lines.extend(
            message
                .content
                .lines()
                .map(|line| Line::raw(line.to_owned())),
        );
        if index + 1 < messages.len() {
            lines.push(Line::raw(""));
        }
    }
    Text::from(lines)
}

fn conversation_height(messages: &[Message], width: usize) -> usize {
    let width = width.max(1);
    messages
        .iter()
        .enumerate()
        .map(|(index, message)| {
            let content_height: usize = message
                .content
                .lines()
                .map(|line| UnicodeWidthStr::width(line).max(1).div_ceil(width))
                .sum();
            1 + content_height + usize::from(index + 1 < messages.len())
        })
        .sum()
}

fn previous_boundary(value: &str, cursor: usize) -> Option<usize> {
    value[..cursor]
        .char_indices()
        .next_back()
        .map(|(index, _)| index)
}

fn visible_input_start(value: &str, cursor: usize, width: usize) -> usize {
    let mut used = 0;
    for (index, character) in value[..cursor].char_indices().rev() {
        let char_width = unicode_width::UnicodeWidthChar::width(character).unwrap_or(0);
        if used + char_width > width {
            return index + character.len_utf8();
        }
        used += char_width;
    }
    0
}

#[cfg(test)]
mod tests {
    use super::{previous_boundary, visible_input_start};

    #[test]
    fn cursor_helpers_respect_utf8() {
        assert_eq!(previous_boundary("a你", 4), Some(1));
        assert_eq!(visible_input_start("ab你好", 8, 4), 2);
    }
}
