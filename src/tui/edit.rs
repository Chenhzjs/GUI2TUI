use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::semantic::{BackendLocator, RuntimeNodeId};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditBuffer {
    text: String,
    /// Cursor position measured in Unicode scalar values, never UTF-8 bytes.
    cursor: usize,
}

impl EditBuffer {
    pub fn new(text: String) -> Self {
        let cursor = text.chars().count();
        Self { text, cursor }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn insert(&mut self, character: char) {
        let byte = byte_index(&self.text, self.cursor);
        self.text.insert(byte, character);
        self.cursor += 1;
    }

    pub fn move_left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn move_right(&mut self) {
        self.cursor = (self.cursor + 1).min(self.text.chars().count());
    }

    pub fn home(&mut self) {
        self.cursor = 0;
    }

    pub fn end(&mut self) {
        self.cursor = self.text.chars().count();
    }

    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let start = byte_index(&self.text, self.cursor - 1);
        let end = byte_index(&self.text, self.cursor);
        self.text.replace_range(start..end, "");
        self.cursor -= 1;
    }

    pub fn delete(&mut self) {
        let count = self.text.chars().count();
        if self.cursor >= count {
            return;
        }
        let start = byte_index(&self.text, self.cursor);
        let end = byte_index(&self.text, self.cursor + 1);
        self.text.replace_range(start..end, "");
    }
}

fn byte_index(text: &str, character_index: usize) -> usize {
    text.char_indices()
        .nth(character_index)
        .map_or(text.len(), |(index, _)| index)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditSession {
    pub target: RuntimeNodeId,
    pub backend_locator: BackendLocator,
    pub original_value: String,
    pub buffer: EditBuffer,
    pub source_generation: u64,
    pub external_change_detected: bool,
    pub commit_pending: bool,
}

impl EditSession {
    pub fn new(
        target: RuntimeNodeId,
        backend_locator: BackendLocator,
        original_value: String,
        source_generation: u64,
    ) -> Self {
        Self {
            target,
            backend_locator,
            buffer: EditBuffer::new(original_value.clone()),
            original_value,
            source_generation,
            external_change_detected: false,
            commit_pending: false,
        }
    }

    pub fn mark_external_change(&mut self) {
        if !self.commit_pending {
            self.external_change_detected = true;
        }
    }

    pub fn can_commit(&self) -> bool {
        !self.external_change_detected && !self.commit_pending
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditCommand {
    Insert(char),
    Left,
    Right,
    Home,
    End,
    Backspace,
    Delete,
    Commit,
    Cancel,
    BlockedTab,
    Quit,
    Ignore,
}

pub fn key_to_edit_command(event: KeyEvent) -> EditCommand {
    if event.kind == KeyEventKind::Release {
        return EditCommand::Ignore;
    }
    match (event.code, event.modifiers) {
        (KeyCode::Char('c'), modifiers) if modifiers.contains(KeyModifiers::CONTROL) => {
            EditCommand::Quit
        }
        (KeyCode::Enter, _) => EditCommand::Commit,
        (KeyCode::Esc, _) => EditCommand::Cancel,
        (KeyCode::Tab | KeyCode::BackTab, _) => EditCommand::BlockedTab,
        (KeyCode::Left, _) => EditCommand::Left,
        (KeyCode::Right, _) => EditCommand::Right,
        (KeyCode::Home, _) => EditCommand::Home,
        (KeyCode::End, _) => EditCommand::End,
        (KeyCode::Backspace, _) => EditCommand::Backspace,
        (KeyCode::Delete, _) => EditCommand::Delete,
        (KeyCode::Char(character), modifiers) if !modifiers.contains(KeyModifiers::CONTROL) => {
            EditCommand::Insert(character)
        }
        _ => EditCommand::Ignore,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_ascii_and_unicode_without_corrupting_utf8() {
        let mut buffer = EditBuffer::new("a中c".to_owned());
        buffer.move_left();
        buffer.backspace();
        buffer.insert('文');
        assert_eq!(buffer.text(), "a文c");
        assert_eq!(buffer.cursor(), 2);
        buffer.home();
        buffer.delete();
        assert_eq!(buffer.text(), "文c");
        buffer.end();
        buffer.insert('好');
        assert_eq!(buffer.text(), "文c好");
    }

    #[test]
    fn edit_keymap_keeps_global_shortcut_letters_as_text() {
        assert_eq!(
            key_to_edit_command(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)),
            EditCommand::Insert('q')
        );
        assert_eq!(
            key_to_edit_command(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE)),
            EditCommand::Insert('r')
        );
        assert_eq!(
            key_to_edit_command(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)),
            EditCommand::BlockedTab
        );
    }

    #[test]
    fn external_change_blocks_commit_but_own_pending_echo_does_not() {
        let locator = BackendLocator::new(":1.2", "/input");
        let mut external = EditSession::new(
            RuntimeNodeId::new(7),
            locator.clone(),
            "alice".to_owned(),
            3,
        );
        external.mark_external_change();
        assert!(!external.can_commit());

        let mut own = EditSession::new(RuntimeNodeId::new(7), locator, "alice".to_owned(), 3);
        own.commit_pending = true;
        own.mark_external_change();
        assert!(!own.external_change_detected);
    }
}
