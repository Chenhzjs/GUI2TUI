use crate::semantic::{
    CollectionCompleteness, RuntimeNodeId, SemanticCache, SemanticRole, collection_completeness,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SemanticTableCell {
    pub source: RuntimeNodeId,
    pub row: usize,
    pub column: usize,
    pub label: String,
    pub row_span: usize,
    pub column_span: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TablePosition {
    pub row: usize,
    pub column: usize,
    pub cell: Option<RuntimeNodeId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SemanticTableModel {
    pub owner: RuntimeNodeId,
    pub rows: Option<usize>,
    pub columns: Option<usize>,
    pub cells: Vec<SemanticTableCell>,
    pub column_headers: Vec<String>,
    pub completeness: CollectionCompleteness,
    pub position: TablePosition,
}

impl SemanticTableModel {
    pub fn analyze(cache: &SemanticCache, owner: RuntimeNodeId) -> Option<Self> {
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
                    append_cell(cache, *cell_id, row, column, &mut cells, &mut headers);
                }
            } else if node.role == SemanticRole::Cell {
                let index = cells.len();
                append_cell(cache, *child, 0, index, &mut cells, &mut headers);
            }
        }
        let columns = cells
            .iter()
            .map(|cell| cell.column + 1)
            .max()
            .filter(|_| !cells.is_empty());
        let completeness = collection_completeness(table);
        let rows = (completeness == CollectionCompleteness::Complete)
            .then_some(realized_rows.max(usize::from(!cells.is_empty())));
        let position = cells
            .first()
            .map_or(TablePosition::default(), |cell| TablePosition {
                row: cell.row,
                column: cell.column,
                cell: Some(cell.source),
            });
        Some(Self {
            owner,
            rows,
            columns,
            cells,
            column_headers: headers,
            completeness,
            position,
        })
    }

    pub fn move_by(&mut self, row_delta: isize, column_delta: isize) {
        if self.cells.is_empty() {
            return;
        }
        let max_row = self.cells.iter().map(|cell| cell.row).max().unwrap_or(0);
        let max_column = self.cells.iter().map(|cell| cell.column).max().unwrap_or(0);
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
            .find(|cell| cell.row == row && cell.column == column)
            .or_else(|| self.cells.iter().find(|cell| cell.row == row))
            .or_else(|| self.cells.first());
        if let Some(cell) = cell {
            self.position = TablePosition {
                row: cell.row,
                column: cell.column,
                cell: Some(cell.source),
            };
        }
    }
}

fn append_cell(
    cache: &SemanticCache,
    id: RuntimeNodeId,
    row: usize,
    column: usize,
    cells: &mut Vec<SemanticTableCell>,
    headers: &mut Vec<String>,
) {
    let Some(node) = cache.node(id) else { return };
    if node.role != SemanticRole::Cell {
        return;
    }
    let label = node
        .name
        .clone()
        .or_else(|| node.value.clone())
        .unwrap_or_else(|| "[empty]".to_owned());
    if node.debug.atspi_role.contains("header") {
        headers.push(label.clone());
    }
    cells.push(SemanticTableCell {
        source: id,
        row,
        column,
        label,
        row_span: 1,
        column_span: 1,
    });
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
        assert_eq!(model.columns, Some(2));
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
}
