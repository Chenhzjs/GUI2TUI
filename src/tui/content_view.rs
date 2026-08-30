use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::{
    content::{ContentBlockId, ContentSearchResult, ReaderBlock},
    semantic::RuntimeNodeId,
    transcompile::SceneElementId,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContentViewMode {
    Reader,
    Outline,
    Search,
}

#[derive(Clone, Debug)]
pub struct ContentViewState {
    pub root: RuntimeNodeId,
    pub position: ContentBlockId,
    pub mode: ContentViewMode,
    pub reader_blocks: Vec<ReaderBlock>,
    pub outline_selected: usize,
    pub query: String,
    pub results: Vec<ContentSearchResult>,
    pub result_selected: usize,
    pub restore_scene: Option<SceneElementId>,
    pub restore_runtime: Option<RuntimeNodeId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContentViewCommand {
    Continue,
    Close,
    MoveBlocks(isize),
    OpenOutline,
    OpenSearch,
    OutlineMove(isize),
    ChooseOutline,
    SearchChanged,
    SearchMove(isize),
    ChooseSearch,
}

impl ContentViewState {
    pub fn new(
        root: RuntimeNodeId,
        position: ContentBlockId,
        restore_scene: Option<SceneElementId>,
        restore_runtime: Option<RuntimeNodeId>,
    ) -> Self {
        Self {
            root,
            position,
            mode: ContentViewMode::Reader,
            reader_blocks: Vec::new(),
            outline_selected: 0,
            query: String::new(),
            results: Vec::new(),
            result_selected: 0,
            restore_scene,
            restore_runtime,
        }
    }

    pub fn handle_key(&mut self, event: KeyEvent) -> ContentViewCommand {
        if event.kind == KeyEventKind::Release {
            return ContentViewCommand::Continue;
        }
        match self.mode {
            ContentViewMode::Reader => match event.code {
                KeyCode::Esc => ContentViewCommand::Close,
                KeyCode::Up | KeyCode::Char('k') => ContentViewCommand::MoveBlocks(-1),
                KeyCode::Down | KeyCode::Char('j') => ContentViewCommand::MoveBlocks(1),
                KeyCode::PageUp => ContentViewCommand::MoveBlocks(-10),
                KeyCode::PageDown => ContentViewCommand::MoveBlocks(10),
                KeyCode::Char('o') => ContentViewCommand::OpenOutline,
                KeyCode::Char('/') => ContentViewCommand::OpenSearch,
                _ => ContentViewCommand::Continue,
            },
            ContentViewMode::Outline => match event.code {
                KeyCode::Esc => {
                    self.mode = ContentViewMode::Reader;
                    ContentViewCommand::Continue
                }
                KeyCode::Up | KeyCode::Char('k') => ContentViewCommand::OutlineMove(-1),
                KeyCode::Down | KeyCode::Char('j') => ContentViewCommand::OutlineMove(1),
                KeyCode::Enter => ContentViewCommand::ChooseOutline,
                KeyCode::Char('/') => ContentViewCommand::OpenSearch,
                _ => ContentViewCommand::Continue,
            },
            ContentViewMode::Search => match (event.code, event.modifiers) {
                (KeyCode::Esc, _) => {
                    self.mode = ContentViewMode::Reader;
                    ContentViewCommand::Continue
                }
                (KeyCode::Enter, _) => ContentViewCommand::ChooseSearch,
                (KeyCode::Up, _) => ContentViewCommand::SearchMove(-1),
                (KeyCode::Down, _) => ContentViewCommand::SearchMove(1),
                (KeyCode::Backspace, _) => {
                    self.query.pop();
                    ContentViewCommand::SearchChanged
                }
                (KeyCode::Char(character), modifiers)
                    if !modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    self.query.push(character);
                    ContentViewCommand::SearchChanged
                }
                _ => ContentViewCommand::Continue,
            },
        }
    }
}

pub fn move_index(current: usize, delta: isize, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    current.saturating_add_signed(delta).min(len - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn reader_outline_search_are_distinct_terminal_lifecycles() {
        let mut state =
            ContentViewState::new(RuntimeNodeId::new(1), ContentBlockId::new(1), None, None);
        assert_eq!(
            state.handle_key(key(KeyCode::Char('o'))),
            ContentViewCommand::OpenOutline
        );
        state.mode = ContentViewMode::Outline;
        assert_eq!(
            state.handle_key(key(KeyCode::Esc)),
            ContentViewCommand::Continue
        );
        assert_eq!(state.mode, ContentViewMode::Reader);
        state.mode = ContentViewMode::Search;
        assert_eq!(
            state.handle_key(key(KeyCode::Char('界'))),
            ContentViewCommand::SearchChanged
        );
        assert_eq!(state.query, "界");
    }

    #[test]
    fn bounded_navigation_does_not_wrap_document_position() {
        assert_eq!(move_index(0, -1, 4), 0);
        assert_eq!(move_index(3, 1, 4), 3);
        assert_eq!(move_index(1, 1, 4), 2);
    }
}
