use crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

use super::action::UiIntent;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseIntent {
    Click { x: u16, y: u16 },
    Scroll(i16),
}

pub fn key_to_intent(event: KeyEvent) -> Option<UiIntent> {
    if event.kind == KeyEventKind::Release {
        return None;
    }
    match (event.code, event.modifiers) {
        (KeyCode::BackTab, _) | (KeyCode::Tab, KeyModifiers::SHIFT) => {
            Some(UiIntent::FocusPrevious)
        }
        (KeyCode::Tab, _) => Some(UiIntent::FocusNext),
        (KeyCode::Enter, _) => Some(UiIntent::Activate),
        (KeyCode::Char(':'), _) => Some(UiIntent::OpenCommandPalette),
        (KeyCode::Char(' '), _) => Some(UiIntent::Toggle),
        (KeyCode::Char('r'), _) => Some(UiIntent::Refresh),
        (KeyCode::Char('q'), _) | (KeyCode::Esc, _) => Some(UiIntent::Quit),
        (KeyCode::Up, _) => Some(UiIntent::ScrollLines(-1)),
        (KeyCode::Down, _) => Some(UiIntent::ScrollLines(1)),
        (KeyCode::PageUp, _) => Some(UiIntent::ScrollPages(-1)),
        (KeyCode::PageDown, _) => Some(UiIntent::ScrollPages(1)),
        _ => None,
    }
}

pub fn mouse_to_intent(event: MouseEvent) -> Option<MouseIntent> {
    match event.kind {
        MouseEventKind::Down(MouseButton::Left) => Some(MouseIntent::Click {
            x: event.column,
            y: event.row,
        }),
        MouseEventKind::ScrollUp => Some(MouseIntent::Scroll(-3)),
        MouseEventKind::ScrollDown => Some(MouseIntent::Scroll(3)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_keyboard_to_semantic_intents() {
        assert_eq!(
            key_to_intent(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)),
            Some(UiIntent::FocusNext)
        );
        assert_eq!(
            key_to_intent(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT)),
            Some(UiIntent::FocusPrevious)
        );
        assert_eq!(
            key_to_intent(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            Some(UiIntent::Activate)
        );
    }
}
