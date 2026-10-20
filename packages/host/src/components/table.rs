//! `Table`: gpui-component's `DataTable` (virtualized) over the node's `columns` and `rows`
//! props. Cells are text. One retained `TableState` per node; its delegate reads the rows from
//! the tree when it draws them, so big tables are not copied on every sync.

use std::collections::{HashMap, HashSet};

use gpui_kit::component::table::{
    Column, ColumnSort, DataTable, TableDelegate, TableEvent, TableState,
};
use gpui_kit::*;
use serde_json::{Value, json};

use super::{chosen, emit, prop_bool};
use crate::Session;
use crate::protocol::NodeId;
use crate::schema::NodeType;
use crate::tree::{Node, Tree};

pub struct Delegate {
    session: Entity<Session>,
    id: NodeId,
}

impl Delegate {
    fn prop<'a>(&self, cx: &'a App, key: &str) -> &'a [Value] {
        let node = self.session.read(cx).tree.node(self.id);
        node.and_then(|node| node.props.get(key))
            .and_then(Value::as_array)
            .map_or(&[], Vec::as_slice)
    }
}

impl TableDelegate for Delegate {
    fn columns_count(&self, cx: &App) -> usize {
        self.prop(cx, "columns").len()
    }

    fn rows_count(&self, cx: &App) -> usize {
        self.prop(cx, "rows").len()
    }

    fn column(&self, col_ix: usize, cx: &App) -> Column {
        let spec = &self.prop(cx, "columns")[col_ix];
        let text = |key| {
            spec.get(key)
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned()
        };
        let mut column = Column::new(text("key"), text("name"));
        if let Some(width) = spec.get("width").and_then(Value::as_f64) {
            column = column.width(px(width as f32));
        }
        match spec.get("align").and_then(Value::as_str) {
            Some("center") => column = column.text_center(),
            Some("right") => column = column.text_right(),
            _ => {}
        }
        if spec.get("sortable") == Some(&Value::Bool(true)) {
            column = column.sortable();
        }
        column
    }

    fn perform_sort(
        &mut self,
        col_ix: usize,
        sort: ColumnSort,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) {
        let tree = &self.session.read(cx).tree;
        let key = self
            .prop(cx, "columns")
            .get(col_ix)
            .and_then(|spec| spec.get("key"))
            .and_then(Value::as_str);
        let direction = match sort {
            ColumnSort::Ascending => "asc",
            ColumnSort::Descending => "desc",
            ColumnSort::Default => "none",
        };
        sorted(tree, self.id, key.unwrap_or(""), direction);
    }

    fn render_th(
        &mut self,
        col_ix: usize,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let spec = self.prop(cx, "columns").get(col_ix);
        let name = spec
            .and_then(|spec| spec.get("name"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        aligned(spec).child(name)
    }

    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let spec = self.prop(cx, "columns").get(col_ix);
        let key = spec
            .and_then(|spec| spec.get("key"))
            .and_then(Value::as_str);
        let cell = key.and_then(|key| self.prop(cx, "rows").get(row_ix)?.get(key));
        aligned(spec).child(cell.map(cell_text).unwrap_or_default())
    }
}

/// A cell laid out by its column's `align` (DataTable leaves alignment to the delegate).
fn aligned(spec: Option<&Value>) -> Div {
    let cell = div().size_full().flex().items_center();
    match spec
        .and_then(|spec| spec.get("align"))
        .and_then(Value::as_str)
    {
        Some("center") => cell.justify_center(),
        Some("right") => cell.justify_end(),
        _ => cell,
    }
}

fn cell_text(cell: &Value) -> String {
    match cell {
        Value::String(text) => text.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

pub struct TableSlot {
    state: Entity<TableState<Delegate>>,
    /// The `columns` the state was built from: it refreshes when they change.
    columns: Value,
    _events: Subscription,
}

impl TableSlot {
    /// `focus` on a Table: the arrow keys then move the selection.
    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        self.state.read(cx).focus_handle(cx).focus(window, cx);
    }
}

/// Each Table among `members`: its columns and the row to select (dropped when out of range).
pub struct TableSnapshot(Vec<(NodeId, Value, Option<usize>)>);

pub fn snapshot(tree: &Tree, members: &HashSet<NodeId>) -> TableSnapshot {
    TableSnapshot(
        tree.nodes()
            .filter(|(id, node)| node.node_type == NodeType::Table && members.contains(id))
            .map(|(id, node)| {
                let rows = node
                    .props
                    .get("rows")
                    .and_then(Value::as_array)
                    .map_or(0, Vec::len);
                let selected = node
                    .props
                    .get("selected")
                    .and_then(Value::as_u64)
                    .map(|ix| ix as usize)
                    .filter(|&ix| ix < rows);
                (
                    id,
                    node.props.get("columns").cloned().unwrap_or(Value::Null),
                    selected,
                )
            })
            .collect(),
    )
}

/// Creates states, drops removed ones, refreshes changed columns and shows each node's
/// selection (which also reverts a controlled Table's own selection until the client confirms it).
pub fn sync<V: 'static>(
    slots: &mut HashMap<NodeId, TableSlot>,
    snapshot: TableSnapshot,
    session: &Entity<Session>,
    window: &mut Window,
    cx: &mut Context<V>,
) {
    slots.retain(|id, _| snapshot.0.iter().any(|(table, ..)| table == id));
    for (id, columns, selected) in snapshot.0 {
        match slots.get_mut(&id) {
            None => {
                let delegate = Delegate {
                    session: session.clone(),
                    id,
                };
                // Moving columns is off: the cells follow the order of `columns`.
                let state = cx.new(|cx| {
                    TableState::new(delegate, window, cx)
                        .col_movable(false)
                        .col_selectable(false)
                });
                let session = session.clone();
                let _events =
                    cx.subscribe_in(&state, window, move |_, _, event: &TableEvent, _, cx| {
                        session.update(cx, |session, cx| {
                            // As `snapshot` reads it: a `selected` past the end selects nothing, so clearing it is no change.
                            let node = session.tree.node(id);
                            let rows = node
                                .and_then(|node| node.props.get("rows"))
                                .and_then(Value::as_array)
                                .map_or(0, Vec::len);
                            let current = node
                                .and_then(|node| node.props.get("selected"))
                                .and_then(Value::as_u64)
                                .filter(|&ix| ix < rows as u64);
                            match *event {
                                TableEvent::SelectRow(ix) if current != Some(ix as u64) => {
                                    row_selected(&mut session.tree, id, Some(ix));
                                }
                                TableEvent::ClearSelection if current.is_some() => {
                                    row_selected(&mut session.tree, id, None);
                                }
                                TableEvent::DoubleClickedRow(ix) => {
                                    row_double_clicked(&session.tree, id, ix)
                                }
                                _ => return,
                            }
                            // Synced or not, re-sync: a controlled Table goes back to the client's row.
                            cx.notify();
                        });
                    });
                slots.insert(
                    id,
                    TableSlot {
                        state,
                        columns,
                        _events,
                    },
                );
            }
            Some(slot) if slot.columns != columns => {
                slot.columns = columns;
                slot.state.update(cx, |state, cx| state.refresh(cx));
            }
            Some(_) => {}
        }
        let state = &slots[&id].state;
        // Rows may have changed: draw them again.
        state.update(cx, |state, cx| {
            if state.selected_row() != selected {
                match selected {
                    Some(ix) => state.set_selected_row(ix, cx),
                    None => state.clear_selection(cx),
                }
            }
            cx.notify();
        });
    }
}

/// Props: `columns`, `rows` and `selected` (read by the delegate and `sync`), `stripe`, `bordered`.
pub fn build(node: &Node, slot: &TableSlot) -> DataTable<Delegate> {
    let bordered = node
        .props
        .get("bordered")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    DataTable::new(&slot.state)
        .stripe(prop_bool(node, "stripe"))
        .bordered(bordered)
}

// User actions, shared with `simulate`.

/// A row selected with a click or the arrow keys (`None`: Escape cleared it).
pub fn row_selected(tree: &mut Tree, id: NodeId, ix: Option<usize>) -> bool {
    chosen(tree, id, "selected", ix.map_or(Value::Null, Value::from))
}

pub fn row_double_clicked(tree: &Tree, id: NodeId, ix: usize) {
    emit(tree, id, "doubleClick", Some(ix.into()), None);
}

/// A click on a sortable column's sort button: gpui-component cycles none → desc → asc → none.
/// The app sorts `rows` itself.
pub fn sorted(tree: &Tree, id: NodeId, column: &str, direction: &str) {
    emit(
        tree,
        id,
        "sort",
        Some(json!({"column": column, "direction": direction})),
        None,
    );
}

/// Checks `columns`: objects with `key` and `name` strings, and optional `width` (above 0),
/// `align` and `sortable`.
pub fn check_columns(value: &Value) -> Result<(), String> {
    let columns = value.as_array().ok_or("a list of columns")?;
    for (ix, column) in columns.iter().enumerate() {
        let optional =
            |key, ok: fn(&Value) -> bool| column.get(key).is_none_or(|v| v.is_null() || ok(v));
        let ok = column.get("key").is_some_and(Value::is_string)
            && column.get("name").is_some_and(Value::is_string)
            && optional("width", |v| {
                v.as_f64().is_some_and(|w| w.is_finite() && w > 0.)
            })
            && optional("align", |v| {
                matches!(v.as_str(), Some("left" | "center" | "right"))
            })
            && optional("sortable", Value::is_boolean);
        if !ok {
            return Err(format!(
                "column {ix} needs 'key' and 'name' strings (and optional 'width' above 0, 'align' left, center or right, boolean 'sortable')"
            ));
        }
    }
    Ok(())
}

/// Checks `rows`: objects whose cells are strings, numbers, booleans or null.
pub fn check_rows(value: &Value) -> Result<(), String> {
    let rows = value.as_array().ok_or("a list of rows")?;
    for (ix, row) in rows.iter().enumerate() {
        let ok = row.as_object().is_some_and(|cells| {
            cells
                .values()
                .all(|cell| !cell.is_array() && !cell.is_object())
        });
        if !ok {
            return Err(format!(
                "row {ix} must be an object of text, number, boolean or null cells"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    // Not `super::*`: gpui's own `test` attribute would shadow the standard one.
    use super::{cell_text, check_columns, check_rows};
    use serde_json::{Value, json};

    #[test]
    fn columns_and_rows_are_checked() {
        assert!(
            check_columns(
                &json!([{"key": "a", "name": "A", "width": 80, "align": "right", "sortable": true}])
            )
            .is_ok()
        );
        assert!(check_columns(&json!([{"key": "a"}])).is_err());
        assert!(check_columns(&json!([{"key": "a", "name": "A", "width": 0}])).is_err());
        assert!(check_columns(&json!([{"key": "a", "name": "A", "align": "middle"}])).is_err());
        assert!(check_rows(&json!([{"a": "x", "b": 2, "c": null}])).is_ok());
        assert!(check_rows(&json!([["x"]])).is_err());
        assert!(check_rows(&json!([{"a": {"nested": 1}}])).is_err());
        assert_eq!(cell_text(&json!(12.5)), "12.5");
        assert_eq!(cell_text(&Value::Null), "");
    }
}
