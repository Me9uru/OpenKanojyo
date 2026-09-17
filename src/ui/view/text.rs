use ratatui::{
    layout::Alignment,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
};
use unicode_width::UnicodeWidthStr;

use crate::ui::message::{Message, Role};

pub(in crate::ui) fn conversation_text(messages: &[Message], width: usize) -> Text<'static> {
    let mut lines = Vec::new();
    for message in messages {
        let (name, label_color, bubble_style, alignment) = match message.role {
            Role::System => (
                "● 系统",
                Color::Yellow,
                Style::default().fg(Color::Yellow),
                Alignment::Left,
            ),
            Role::User => (
                "你 ●",
                Color::Green,
                Style::default().fg(Color::White).bg(Color::Blue),
                Alignment::Right,
            ),
            Role::Assistant => (
                "● AI",
                Color::Cyan,
                Style::default().fg(Color::White).bg(Color::DarkGray),
                Alignment::Left,
            ),
        };

        let side_margin = usize::from(width > 2);
        let max_outer_width = bubble_max_width(width.saturating_sub(side_margin));
        let padding = usize::from(max_outer_width >= 3);
        let content_width = max_outer_width.saturating_sub(padding * 2).max(1);
        let wrapped = wrap_message(&message.content, content_width);
        let bubble_content_width = wrapped
            .iter()
            .map(|line| UnicodeWidthStr::width(line.as_str()))
            .max()
            .unwrap_or(0)
            .min(content_width);

        lines.push(
            Line::from(Span::styled(
                name,
                Style::default()
                    .fg(label_color)
                    .add_modifier(Modifier::BOLD),
            ))
            .alignment(alignment),
        );

        for content in wrapped {
            let visual_width = UnicodeWidthStr::width(content.as_str()).min(content_width);
            let trailing_padding = bubble_content_width.saturating_sub(visual_width) + padding;
            let bubble = format!(
                "{}{}{}",
                " ".repeat(padding),
                content,
                " ".repeat(trailing_padding)
            );
            let spans = match message.role {
                Role::Assistant | Role::System => vec![
                    Span::raw(" ".repeat(side_margin)),
                    Span::styled(bubble, bubble_style),
                ],
                Role::User => vec![
                    Span::styled(bubble, bubble_style),
                    Span::raw(" ".repeat(side_margin)),
                ],
            };
            lines.push(Line::from(spans).alignment(alignment));
        }
    }
    Text::from(lines)
}

fn bubble_max_width(width: usize) -> usize {
    width.saturating_mul(3).div_ceil(4).max(1).min(width)
}

pub(in crate::ui) fn wrap_message(content: &str, width: usize) -> Vec<String> {
    let mut wrapped = Vec::new();
    for source_line in content.split('\n') {
        let mut line = String::new();
        let mut line_width = 0;
        for character in source_line.chars() {
            let character_width = unicode_width::UnicodeWidthChar::width(character).unwrap_or(0);
            if !line.is_empty() && line_width + character_width > width {
                wrapped.push(line);
                line = String::new();
                line_width = 0;
            }
            line.push(character);
            line_width += character_width;
        }
        wrapped.push(line);
    }
    wrapped
}

pub(in crate::ui) fn visible_input_start(value: &str, cursor: usize, width: usize) -> usize {
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
    use ratatui::{
        buffer::Buffer,
        layout::Rect,
        widgets::{Paragraph, Widget, Wrap},
    };

    use super::{bubble_max_width, conversation_text, visible_input_start, wrap_message};
    use crate::ui::message::{Message, Role};

    #[test]
    fn visible_input_respects_utf8() {
        assert_eq!(visible_input_start("ab你好", 8, 4), 2);
    }

    #[test]
    fn chat_bubble_wraps_by_terminal_width() {
        assert_eq!(bubble_max_width(20), 15);
        assert_eq!(wrap_message("你好 Rust", 5), ["你好 ", "Rust"]);
        assert_eq!(
            wrap_message("第一行\n\n第三行", 10),
            ["第一行", "", "第三行"]
        );
    }

    #[test]
    fn conversation_does_not_insert_blank_lines_between_messages() {
        let messages = [
            Message::new(Role::User, "第一条"),
            Message::new(Role::Assistant, "第二条"),
        ];

        let text = conversation_text(&messages, 80);

        assert_eq!(text.lines.len(), 4);
        assert_eq!(text.lines[0].to_string(), "你 ●");
        assert!(text.lines[1].to_string().contains("第一条"));
        assert_eq!(text.lines[2].to_string(), "● AI");
        assert!(text.lines[3].to_string().contains("第二条"));

        let area = Rect::new(0, 0, 80, 4);
        let mut buffer = Buffer::empty(area);
        Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .render(area, &mut buffer);
        let rendered_rows = (0..area.height)
            .map(|y| {
                (0..area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>();

        assert!(rendered_rows.iter().all(|row| !row.trim().is_empty()));
        assert!(rendered_rows[0].contains('你'));
        assert!(rendered_rows[1].contains('一'));
        assert!(rendered_rows[2].contains("● AI"));
        assert!(rendered_rows[3].contains('二'));
    }
}
