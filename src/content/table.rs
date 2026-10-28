use std::collections::{BTreeMap, HashMap};
use std::ops::Deref;

use crate::semantic::{
    CollectionCompleteness, RuntimeNodeId, SemanticCache, SemanticRole, collection_completeness,
};

/// Read evidence supplied by the observation owner, never by terminal layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TablePositionRead {
    Known {
        row: usize,
        column: usize,
        row_span: Option<usize>,
        column_span: Option<usize>,
    },
    Unavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TablePositionSource {
    PublicTableCell,
    SemanticRowOrder,
    NotRequested,
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SemanticTableCell {
    pub source: RuntimeNodeId,
    pub row: Option<usize>,
    pub column: Option<usize>,
    pub position_source: TablePositionSource,
    pub label: String,
    pub label_source: Option<RuntimeNodeId>,
    // Unknown extents remain None; a layout unit is not a public span.
    pub row_span: Option<usize>,
    pub column_span: Option<usize>,
    pub selected: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TablePosition {
    pub row: usize,
    pub column: usize,
    pub cell: Option<RuntimeNodeId>,
}

/// Shared logical structure. Human navigation state is deliberately absent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SemanticTableStructure {
    pub owner: RuntimeNodeId,
    pub rows: Option<usize>,
    pub columns: Option<usize>,
    pub cells: Vec<SemanticTableCell>,
    pub column_headers: Vec<String>,
    pub completeness: CollectionCompleteness,
    pub row_selection: bool,
}

impl SemanticTableStructure {
    pub fn analyze(
        cache: &SemanticCache,
        owner: RuntimeNodeId,
        positions: &BTreeMap<RuntimeNodeId, TablePositionRead>,
    ) -> Option<Self> {
        let table = cache.node(owner)?;
        if table.role != SemanticRole::Table {
            return None;
        }
        let mut cells = Vec::new();
        let mut headers = Vec::new();
        let mut realized_rows = 0;
        for child in &table.children {
            let Some(node) = cache.node(*child) else {
                continue;
            };
            if node.role == SemanticRole::Row {
                let row = realized_rows;
                realized_rows += 1;
                for (column, cell_id) in node.children.iter().enumerate() {
                    append_cell(
                        cache,
                        *cell_id,
                        Some((row, column)),
                        positions,
                        &mut cells,
                        &mut headers,
                    );
                }
            } else if node.role == SemanticRole::Cell {
                append_cell(cache, *child, None, positions, &mut cells, &mut headers);
            }
        }
        let completeness = collection_completeness(table);
        // A realized extent is not a logical total for a partial collection.
        let positioned = cells
            .iter()
            .all(|cell| cell.row.is_some() && cell.column.is_some());
        let complete = completeness == CollectionCompleteness::Complete
            && positioned
            && cells
                .iter()
                .all(|c| c.row_span.is_some() && c.column_span.is_some());
        let rows = complete.then(|| {
            cells
                .iter()
                .filter_map(|c| c.row.zip(c.row_span).map(|(r, s)| r + s))
                .max()
                .unwrap_or(0)
        });
        let columns = complete.then(|| {
            cells
                .iter()
                .filter_map(|c| c.column.zip(c.column_span).map(|(c, s)| c + s))
                .max()
                .unwrap_or(0)
        });
        Some(Self {
            owner,
            rows,
            columns,
            cells,
            column_headers: headers,
            completeness,
            row_selection: table
                .capabilities
                .contains(&crate::semantic::SemanticCapability::SelectCurrentTableRow),
        })
    }
}

/// Human cursor over a copy of the shared structure; no second organizer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SemanticTableModel {
    pub structure: SemanticTableStructure,
    pub position: TablePosition,
}

impl Deref for SemanticTableModel {
    type Target = SemanticTableStructure;
    fn deref(&self) -> &Self::Target {
        &self.structure
    }
}

impl SemanticTableModel {
    pub fn from_structure(structure: SemanticTableStructure) -> Self {
        let position = structure
            .cells
            .first()
            .map_or(TablePosition::default(), |cell| TablePosition {
                row: cell.row.unwrap_or(0),
                column: cell.column.unwrap_or(0),
                cell: Some(cell.source),
            });
        Self {
            structure,
            position,
        }
    }

    pub fn analyze(cache: &SemanticCache, owner: RuntimeNodeId) -> Option<Self> {
        SemanticTableStructure::analyze(cache, owner, &BTreeMap::new()).map(Self::from_structure)
    }

    pub fn analyze_with_positions(
        cache: &SemanticCache,
        owner: RuntimeNodeId,
        current_positions: &HashMap<RuntimeNodeId, (usize, usize)>,
    ) -> Option<Self> {
        let positions = current_positions
            .iter()
            .map(|(id, (row, column))| {
                (
                    *id,
                    TablePositionRead::Known {
                        row: *row,
                        column: *column,
                        row_span: None,
                        column_span: None,
                    },
                )
            })
            .collect();
        SemanticTableStructure::analyze(cache, owner, &positions).map(Self::from_structure)
    }

    pub fn move_by(&mut self, row_delta: isize, column_delta: isize) {
        if self.cells.is_empty() {
            return;
        }
        // Missing positions do not make exposed cells disappear. Navigate their
        // stable structural order without asserting a GUI row or column.
        if self
            .cells
            .iter()
            .any(|cell| cell.row.is_none() || cell.column.is_none())
        {
            let current = self
                .cells
                .iter()
                .position(|c| Some(c.source) == self.position.cell)
                .unwrap_or(0);
            let next = current
                .saturating_add_signed(if row_delta != 0 {
                    row_delta
                } else {
                    column_delta
                })
                .min(self.cells.len() - 1);
            let cell = &self.cells[next];
            self.position = TablePosition {
                row: cell.row.unwrap_or(0),
                column: cell.column.unwrap_or(0),
                cell: Some(cell.source),
            };
            return;
        }
        let max_row = self
            .cells
            .iter()
            .filter_map(|cell| cell.row)
            .max()
            .unwrap_or(0);
        let max_column = self
            .cells
            .iter()
            .filter_map(|cell| cell.column)
            .max()
            .unwrap_or(0);
        let row = self
            .position
            .row
            .saturating_add_signed(row_delta)
            .min(max_row);
        let column = self
            .position
            .column
            .saturating_add_signed(column_delta)
            .min(max_column);
        let cell = self
            .cells
            .iter()
            .find(|cell| cell.row == Some(row) && cell.column == Some(column))
            .or_else(|| self.cells.iter().find(|cell| cell.row == Some(row)))
            .or_else(|| self.cells.first());
        if let Some(cell) = cell {
            self.position = TablePosition {
                row: cell.row.unwrap_or(0),
                column: cell.column.unwrap_or(0),
                cell: Some(cell.source),
            };
        }
    }
}

fn append_cell(
    cache: &SemanticCache,
    id: RuntimeNodeId,
    semantic_position: Option<(usize, usize)>,
    positions: &BTreeMap<RuntimeNodeId, TablePositionRead>,
    cells: &mut Vec<SemanticTableCell>,
    headers: &mut Vec<String>,
) {
    let Some(node) = cache.node(id) else { return };
    if node.role != SemanticRole::Cell {
        return;
    }
    let (position, position_source) = match positions.get(&id) {
        Some(TablePositionRead::Known { row, column, .. }) => {
            (Some((*row, *column)), TablePositionSource::PublicTableCell)
        }
        Some(TablePositionRead::Unavailable) => (
            semantic_position,
            if semantic_position.is_some() {
                TablePositionSource::SemanticRowOrder
            } else {
                TablePositionSource::Unavailable
            },
        ),
        None => (
            semantic_position,
            if semantic_position.is_some() {
                TablePositionSource::SemanticRowOrder
            } else {
                TablePositionSource::NotRequested
            },
        ),
    };
    let label = cell_label(cache, id);
    let label_source = label.as_ref().map(|(id, _)| *id);
    let label = label.map_or_else(|| "[empty]".to_owned(), |(_, label)| label);
    if node.debug.atspi_role.contains("header") {
        headers.push(label.clone());
    }
    cells.push(SemanticTableCell {
        source: id,
        row: position.map(|p| p.0),
        column: position.map(|p| p.1),
        position_source,
        label,
        label_source,
        row_span: match positions.get(&id) {
            Some(TablePositionRead::Known { row_span, .. }) => *row_span,
            _ => None,
        },
        column_span: match positions.get(&id) {
            Some(TablePositionRead::Known { column_span, .. }) => *column_span,
            _ => None,
        },
        selected: node
            .states
            .contains(&crate::semantic::SemanticState::Selected),
    });
}

fn cell_label(cache: &SemanticCache, id: RuntimeNodeId) -> Option<(RuntimeNodeId, String)> {
    let node = cache.node(id)?;
    node.name
        .clone()
        .filter(|label| !label.trim().is_empty())
        .or_else(|| node.value.clone())
        .filter(|label| !label.trim().is_empty())
        .map(|label| (id, label))
        .or_else(|| {
            node.children
                .iter()
                .find_map(|child| cell_label(cache, *child))
        })
}

#[cfg(test)]
mod tests {
    use crate::semantic::{BackendLocator, DebugInfo, SemanticNode, SemanticState};

    use super::*;

    fn node(id: u64, role: SemanticRole, name: &str) -> SemanticNode {
        SemanticNode {
            runtime_id: RuntimeNodeId::new(id),
            backend_locator: BackendLocator::new(":1.8", format!("/node/{id}")),
            index_in_parent: None,
            role,
            name: Some(name.to_owned()),
            description: None,
            value: None,
            text_input_kind: None,
            states: Vec::new(),
            actions: Vec::new(),
            capabilities: Vec::new(),
            children: Vec::new(),
            truncations: Vec::new(),
            debug: DebugInfo::default(),
        }
    }

    #[test]
    fn small_table_has_semantic_cells_and_navigation() {
        let mut table = node(1, SemanticRole::Table, "Scores");
        let mut row = node(2, SemanticRole::Row, "Alice row");
        row.children.push(node(3, SemanticRole::Cell, "Alice"));
        row.children.push(node(4, SemanticRole::Cell, "92"));
        table.children.push(row);
        let cache = SemanticCache::from_snapshot(table).unwrap();
        let mut model = SemanticTableModel::analyze(&cache, cache.root_id()).unwrap();
        assert_eq!(model.rows, None);
        assert_eq!(model.columns, None);
        model.move_by(0, 1);
        assert_eq!(
            model.position.cell,
            cache
                .nodes()
                .find(|node| node.name.as_deref() == Some("92"))
                .map(|node| node.runtime_id)
        );
    }

    #[test]
    fn managed_table_never_claims_realized_rows_as_logical_total() {
        let mut table = node(1, SemanticRole::Table, "Large");
        table
            .states
            .push(SemanticState::Other("manages-descendants".to_owned()));
        let cache = SemanticCache::from_snapshot(table).unwrap();
        let model = SemanticTableModel::analyze(&cache, cache.root_id()).unwrap();
        assert_eq!(model.rows, None);
        assert_eq!(model.completeness, CollectionCompleteness::PartialRealized);
    }

    #[test]
    fn flat_table_uses_current_public_positions_and_nested_cell_text() {
        let mut table = node(1, SemanticRole::Table, "Files");
        let mut alpha = node(2, SemanticRole::Cell, "");
        alpha
            .children
            .push(node(3, SemanticRole::Cell, "alpha.txt"));
        let size = node(4, SemanticRole::Cell, "5 bytes");
        let mut beta = node(5, SemanticRole::Cell, "");
        beta.children.push(node(6, SemanticRole::Cell, "beta.txt"));
        table.children.extend([alpha, size, beta]);
        let cache = SemanticCache::from_snapshot(table).unwrap();
        let direct_cells = cache.node(cache.root_id()).unwrap().children.clone();
        let positions = [
            (direct_cells[0], (0, 0)),
            (direct_cells[1], (0, 1)),
            (direct_cells[2], (1, 0)),
        ]
        .into_iter()
        .collect();

        let model = SemanticTableModel::analyze_with_positions(&cache, cache.root_id(), &positions)
            .unwrap();

        assert_eq!(model.cells.len(), 3);
        assert_eq!(model.cells[0].label, "alpha.txt");
        assert_eq!(
            (model.cells[2].row, model.cells[2].column),
            (Some(1), Some(0))
        );
        assert_eq!(model.cells[2].label, "beta.txt");
    }
}
