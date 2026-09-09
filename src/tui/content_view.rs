use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::{
    content::{
        ContentBlockId, ContentSearchResult, ContentSearchSession, ReaderBlock, SearchState,
        SemanticTableModel, VirtualCollectionModel,
    },
    semantic::RuntimeNodeId,
    transcompile::SceneElementId,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContentViewMode {
    Reader,
    Outline,
    Search,
    VirtualCollection,
    Table,
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
    pub full_search: Option<ContentSearchSession>,
    pub virtual_collection: Option<VirtualCollectionModel>,
    pub table: Option<SemanticTableModel>,
    pub table_returns_to_scene: bool,
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
    StartFullSearch,
    CancelFullSearch,
    OpenStructure,
    StructureMove { rows: isize, columns: isize },
    SelectCurrentTableRow,
    ActivateCurrentTableCell,
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
            full_search: None,
            virtual_collection: None,
            table: None,
            table_returns_to_scene: false,
            restore_scene,
            restore_runtime,
        }
    }

    pub fn for_table(
        table: SemanticTableModel,
        restore_scene: Option<SceneElementId>,
        restore_runtime: Option<RuntimeNodeId>,
    ) -> Self {
        Self {
            root: table.owner,
            position: ContentBlockId::new(0),
            mode: ContentViewMode::Table,
            reader_blocks: Vec::new(),
            outline_selected: 0,
            query: String::new(),
            results: Vec::new(),
            result_selected: 0,
            full_search: None,
            virtual_collection: None,
            table: Some(table),
            table_returns_to_scene: true,
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
                KeyCode::Enter => ContentViewCommand::OpenStructure,
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
                    if self
                        .full_search
                        .as_ref()
                        .is_some_and(|search| search.state == SearchState::Running)
                    {
                        ContentViewCommand::CancelFullSearch
                    } else {
                        self.mode = ContentViewMode::Reader;
                        ContentViewCommand::Continue
                    }
                }
                (KeyCode::Char('f'), modifiers) if modifiers.contains(KeyModifiers::CONTROL) => {
                    ContentViewCommand::StartFullSearch
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
            ContentViewMode::VirtualCollection => match event.code {
                KeyCode::Esc => {
                    self.mode = ContentViewMode::Reader;
                    ContentViewCommand::Continue
                }
                KeyCode::Up | KeyCode::Char('k') => ContentViewCommand::StructureMove {
                    rows: -1,
                    columns: 0,
                },
                KeyCode::Down | KeyCode::Char('j') => ContentViewCommand::StructureMove {
                    rows: 1,
                    columns: 0,
                },
                _ => ContentViewCommand::Continue,
            },
            ContentViewMode::Table => match event.code {
                KeyCode::Esc if self.table_returns_to_scene => ContentViewCommand::Close,
                KeyCode::Esc => {
                    self.mode = ContentViewMode::Reader;
                    ContentViewCommand::Continue
                }
                KeyCode::Up | KeyCode::Char('k') => ContentViewCommand::StructureMove {
                    rows: -1,
                    columns: 0,
                },
                KeyCode::Down | KeyCode::Char('j') => ContentViewCommand::StructureMove {
                    rows: 1,
                    columns: 0,
                },
                KeyCode::Left | KeyCode::Char('h') => ContentViewCommand::StructureMove {
                    rows: 0,
                    columns: -1,
                },
                KeyCode::Right | KeyCode::Char('l') => ContentViewCommand::StructureMove {
                    rows: 0,
                    columns: 1,
                },
                KeyCode::Enter if self.table.as_ref().is_some_and(|table| table.row_selection) => {
                    ContentViewCommand::SelectCurrentTableRow
                }
                KeyCode::Char('a') => ContentViewCommand::ActivateCurrentTableCell,
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

    #[test]
    fn full_search_is_explicit_and_escape_cancels_before_leaving_search() {
        let mut state =
            ContentViewState::new(RuntimeNodeId::new(1), ContentBlockId::new(1), None, None);
        state.mode = ContentViewMode::Search;
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL)),
            ContentViewCommand::StartFullSearch
        );
        state.full_search = Some(crate::content::ContentSearchSession {
            id: crate::content::SearchSessionId::new(1),
            root: RuntimeNodeId::new(1),
            query: "needle".to_owned(),
            state: SearchState::Running,
            cursor: 0,
            order: vec![ContentBlockId::new(1)],
            results: Vec::new(),
            progress: crate::content::SearchProgress {
                scanned_blocks: 0,
                total_blocks: Some(1),
                text_rpcs: 0,
            },
            source_offsets: std::collections::HashMap::new(),
        });
        assert_eq!(
            state.handle_key(key(KeyCode::Esc)),
            ContentViewCommand::CancelFullSearch
        );
        assert_eq!(state.mode, ContentViewMode::Search);
    }

    #[test]
    fn table_navigation_is_a_terminal_structure_task() {
        let mut state =
            ContentViewState::new(RuntimeNodeId::new(1), ContentBlockId::new(1), None, None);
        state.mode = ContentViewMode::Table;
        state.table = Some(SemanticTableModel {
            owner: RuntimeNodeId::new(1),
            rows: Some(1),
            columns: Some(1),
            cells: Vec::new(),
            column_headers: Vec::new(),
            completeness: crate::semantic::CollectionCompleteness::Complete,
            row_selection: true,
            position: Default::default(),
        });
        assert_eq!(
            state.handle_key(key(KeyCode::Right)),
            ContentViewCommand::StructureMove {
                rows: 0,
                columns: 1
            }
        );
        assert_eq!(
            state.handle_key(key(KeyCode::Down)),
            ContentViewCommand::StructureMove {
                rows: 1,
                columns: 0
            }
        );
        assert_eq!(
            state.handle_key(key(KeyCode::Enter)),
            ContentViewCommand::SelectCurrentTableRow
        );
        assert_eq!(
            state.handle_key(key(KeyCode::Char('a'))),
            ContentViewCommand::ActivateCurrentTableCell
        );
        let table = state.table.clone().unwrap();
        assert_eq!(
            state.handle_key(key(KeyCode::Esc)),
            ContentViewCommand::Continue
        );
        assert_eq!(state.mode, ContentViewMode::Reader);

        let mut direct = ContentViewState::for_table(table, None, None);
        assert_eq!(
            direct.handle_key(key(KeyCode::Esc)),
            ContentViewCommand::Close
        );
    }
}
