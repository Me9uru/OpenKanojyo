use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};

pub(in crate::ui) enum Action {
    Quit,
    Submit,
    Clear,
    Insert(char),
    Backspace,
    Delete,
    Left,
    Right,
    Home,
    End,
    FollowTail,
    ScrollUp(u16),
    ScrollDown(u16),
}

pub(in crate::ui) fn key_action(key: KeyEvent) -> Option<Action> {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return Some(Action::Quit);
    }
    Some(match key.code {
        KeyCode::Esc => Action::Quit,
        KeyCode::Enter => Action::Submit,
        KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL) => Action::Clear,
        KeyCode::Char(character)
            if !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
        {
            Action::Insert(character)
        }
        KeyCode::Backspace => Action::Backspace,
        KeyCode::Delete => Action::Delete,
        KeyCode::Left => Action::Left,
        KeyCode::Right => Action::Right,
        KeyCode::Home => Action::Home,
        KeyCode::End if key.modifiers.contains(KeyModifiers::CONTROL) => Action::FollowTail,
        KeyCode::End => Action::End,
        KeyCode::Up => Action::ScrollUp(1),
        KeyCode::Down => Action::ScrollDown(1),
        KeyCode::PageUp => Action::ScrollUp(5),
        KeyCode::PageDown => Action::ScrollDown(5),
        _ => return None,
    })
}

pub(in crate::ui) fn mouse_action(mouse: MouseEvent) -> Option<Action> {
    match mouse.kind {
        MouseEventKind::ScrollUp => Some(Action::ScrollUp(3)),
        MouseEventKind::ScrollDown => Some(Action::ScrollDown(3)),
        _ => None,
    }
}
