use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Position, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use unicode_width::UnicodeWidthStr;

use crate::ui::{
    message::Message,
    scroll::ScrollState,
    view::text::{conversation_text, visible_input_start},
};

pub(in crate::ui) struct View<'a> {
    pub messages: &'a [Message],
    pub input: &'a str,
    pub cursor: usize,
    pub busy: bool,
    pub streaming: bool,
    pub buffered_count: usize,
    pub provider_label: &'a str,
}

pub(in crate::ui) fn draw(app: &View<'_>, scroll: &mut ScrollState, frame: &mut Frame) {
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(1),
            Constraint::Length(3),
        ])
        .split(frame.area());

    draw_history(app.messages, scroll, frame, areas[0]);
    draw_status(app, frame, areas[1]);
    draw_input(app, frame, areas[2]);
}

fn draw_history(messages: &[Message], scroll: &mut ScrollState, frame: &mut Frame, area: Rect) {
    let width = area.width.saturating_sub(2) as usize;
    let text = conversation_text(messages, width);
    let inner_height = area.height.saturating_sub(2) as usize;
    let scroll = scroll.resolve(text.height(), inner_height);
    // 气泡已按终端宽度换行；再次 wrap 会让纯空格行增高，导致滚动行数失真。
    let history = Paragraph::new(text)
        .block(
            Block::default()
                .title(" OpenKanojyo · 对话 ")
                .borders(Borders::ALL),
        )
        .scroll((scroll, 0));
    frame.render_widget(history, area);
}

fn draw_status(app: &View<'_>, frame: &mut Frame, area: Rect) {
    let state = if app.streaming {
        "回复中…"
    } else if app.busy {
        "思考中…"
    } else {
        "就绪"
    };
    let buffered = if app.buffered_count == 0 {
        String::new()
    } else {
        format!(" · 已缓冲 {} 条", app.buffered_count)
    };
    let status = Line::from(vec![
        Span::styled(
            format!(" {state}{buffered} "),
            Style::default().fg(Color::Cyan),
        ),
        Span::raw(format!(
            "│ {} │ PgUp/PgDn 滚动  Ctrl+L 清空  Esc 退出",
            app.provider_label,
        )),
    ]);
    frame.render_widget(Paragraph::new(status), area);
}

fn draw_input(app: &View<'_>, frame: &mut Frame, area: Rect) {
    let inner_width = area.width.saturating_sub(2) as usize;
    let start = visible_input_start(app.input, app.cursor, inner_width);
    let input = Paragraph::new(&app.input[start..]).block(
        Block::default()
            .title(" 消息 · /help 命令 ")
            .borders(Borders::ALL),
    );
    frame.render_widget(input, area);

    let cursor_width = UnicodeWidthStr::width(&app.input[start..app.cursor]) as u16;
    frame.set_cursor_position(Position::new(
        area.x.saturating_add(1).saturating_add(cursor_width),
        area.y.saturating_add(1),
    ));
}
