//! `simulate` (`--no-window` only, PROTOCOL.md section 9): plays a user action through the
//! same functions the window's handlers call, so the conformance fixtures can cover events.

use serde::Deserialize;
use serde_json::Value;

use crate::components;
use crate::protocol::{ErrorCode, HostMessage, NodeId, ProtocolError};
use crate::schema::NodeType;
use crate::transport;
use crate::tree::Tree;

#[derive(Deserialize)]
struct Simulate {
    id: Option<NodeId>,
    event: String,
    /// Text, a number for `Slider` and a row for `Table`, or `{column, direction}` for `sort`.
    value: Option<Value>,
}

pub enum Outcome {
    Done,
    /// `windowClosed` was sent: the host must exit, as when the user closes the window.
    Exit,
}

/// `None` when `line` is not a `simulate` message.
pub fn handle(line: &str, tree: &mut Tree) -> Option<Result<Outcome, ProtocolError>> {
    let message: Value = serde_json::from_str(line).ok()?;
    if message.get("op")?.as_str()? != "simulate" {
        return None;
    }
    Some(run(message, tree))
}

/// Acts on the committed tree, like a user acting on what is on screen.
fn run(message: Value, tree: &mut Tree) -> Result<Outcome, ProtocolError> {
    let fail = |code, message: String| ProtocolError {
        code,
        message,
        op: Some("simulate".into()),
    };
    let sim: Simulate = serde_json::from_value(message).map_err(|err| {
        fail(
            ErrorCode::InvalidJson,
            format!("Invalid 'simulate' message: {err}"),
        )
    })?;
    // The user clicking the tray icon or choosing an item of its menu.
    if sim.event == "trayClick" || sim.event == "trayMenu" {
        let tray = tree
            .tray()
            .ok_or_else(|| fail(ErrorCode::InvalidOperation, "There is no tray".into()))?;
        let value = match sim.event.as_str() {
            "trayClick" => None,
            _ => {
                let item = sim
                    .value
                    .as_ref()
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                let item = item.ok_or_else(|| {
                    fail(
                        ErrorCode::InvalidJson,
                        "'trayMenu' needs a string 'value'".into(),
                    )
                })?;
                match tray
                    .menu
                    .as_ref()
                    .and_then(|menu| components::menu::find_item(menu, &item))
                {
                    None => {
                        return Err(fail(
                            ErrorCode::InvalidOperation,
                            format!("The tray menu has no item '{item}'"),
                        ));
                    }
                    Some(false) => return Ok(Outcome::Done),
                    Some(true) => Some(item),
                }
            }
        };
        let event = if value.is_some() { "menu" } else { "click" };
        transport::send(&HostMessage::Tray {
            event: event.into(),
            value,
        });
        return Ok(Outcome::Done);
    }
    // A key pressed in window `id` (1 when absent) with no control taking it: shortcuts.
    if sim.event == "key" {
        let keys = sim
            .value
            .as_ref()
            .and_then(Value::as_str)
            .and_then(crate::keys::parse);
        let keys = keys.ok_or_else(|| {
            fail(
                ErrorCode::InvalidJson,
                "'key' needs a key such as \"ctrl-s\" in 'value'".into(),
            )
        })?;
        crate::keys::pressed(tree, sim.id.unwrap_or(1), &keys);
        return Ok(Outcome::Done);
    }
    // The user closing a window: `id` is the window (1 when absent); the host exits after the last one.
    if sim.event == "windowClosed" {
        let id = sim.id.unwrap_or(1);
        if !tree.close_window(id) {
            return Err(fail(
                ErrorCode::InvalidOperation,
                format!("Window {id} does not exist"),
            ));
        }
        transport::send(&HostMessage::WindowClosed { id });
        // Without a platform, a window counts as open once it has a root. A tray keeps the app running.
        let keep = tree.has_open_window() || tree.tray().is_some();
        return Ok(if keep { Outcome::Done } else { Outcome::Exit });
    }
    let id = sim
        .id
        .ok_or_else(|| fail(ErrorCode::InvalidJson, "'simulate' needs an 'id'".into()))?;
    let node = tree
        .node(id)
        .ok_or_else(|| fail(ErrorCode::UnknownNode, format!("Node {id} does not exist")))?;
    let node_type = node.node_type;
    let prop = |key| {
        node.props
            .get(key)
            .and_then(Value::as_bool)
            .unwrap_or(false)
    };
    let (disabled, checked) = (prop("disabled"), prop("checked"));
    let text = || {
        let text = sim
            .value
            .as_ref()
            .and_then(Value::as_str)
            .map(str::to_owned);
        text.ok_or_else(|| {
            fail(
                ErrorCode::InvalidJson,
                format!("'{}' needs a string 'value'", sim.event),
            )
        })
    };

    let known = matches!(
        (node_type, sim.event.as_str()),
        (NodeType::Div | NodeType::Button, "click")
            | (NodeType::Div, "doubleClick")
            | (NodeType::Div | NodeType::Input, "keyDown")
            | (NodeType::Div, "drop")
            | (NodeType::Dialog, "ok" | "cancel")
            | (NodeType::Checkbox | NodeType::Switch, "change")
            | (NodeType::Input | NodeType::NumberInput, "change" | "submit")
            | (NodeType::OtpInput, "change")
            | (
                NodeType::Textarea
                    | NodeType::RadioGroup
                    | NodeType::Select
                    | NodeType::Slider
                    | NodeType::Tabs,
                "change"
            )
            | (
                NodeType::Rating | NodeType::Pagination | NodeType::Stepper | NodeType::Accordion,
                "change"
            )
            | (NodeType::Calendar, "change")
            | (NodeType::Table, "change" | "doubleClick" | "sort")
            | (NodeType::Tree, "change" | "expand" | "collapse")
            | (NodeType::Diff, "change")
            | (NodeType::Carousel, "change")
            | (NodeType::Div | NodeType::MenuBar, "menu")
            | (NodeType::Alert, "close")
    );
    if !known {
        return Err(fail(
            ErrorCode::InvalidOperation,
            format!("{} has no '{}' event", node_type.name(), sim.event),
        ));
    }
    // GPUI drops events on disabled components; the one rule mirrored here.
    if disabled {
        return Ok(Outcome::Done);
    }
    if sim.event == "menu" {
        let item = text()?;
        let menus = match node_type {
            NodeType::MenuBar => node
                .props
                .get("menus")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|menu| menu.get("items"))
                .collect(),
            _ => node
                .props
                .get("contextMenu")
                .into_iter()
                .collect::<Vec<_>>(),
        };
        return match menus
            .iter()
            .find_map(|items| components::menu::find_item(items, &item))
        {
            None => Err(fail(
                ErrorCode::InvalidOperation,
                format!("Node {id} has no menu item '{item}'"),
            )),
            // A disabled item cannot be chosen.
            Some(false) => Ok(Outcome::Done),
            Some(true) => {
                components::menu::chosen(tree, id, &item);
                Ok(Outcome::Done)
            }
        };
    }
    match (node_type, sim.event.as_str()) {
        (NodeType::Div | NodeType::Button, "click") => components::clicked(tree, id),
        (NodeType::Alert, "close") => components::alert_closed(tree, id),
        // The second click of a double click: it sends `click` too.
        (NodeType::Div, "doubleClick") => {
            components::clicked(tree, id);
            components::double_clicked(tree, id);
        }
        // Answering the open Dialog: OK, or Cancel (also Escape, the X and a click outside).
        (NodeType::Dialog, "ok" | "cancel") => {
            if node.props.get("open") != Some(&Value::Bool(true)) {
                return Err(fail(
                    ErrorCode::InvalidOperation,
                    format!("Dialog {id} is not open"),
                ));
            }
            components::dialog_answered(tree, id, &sim.event);
        }
        (NodeType::Div, "drop") => {
            let paths = sim
                .value
                .as_ref()
                .and_then(Value::as_array)
                .and_then(|paths| {
                    paths
                        .iter()
                        .map(|path| path.as_str().map(str::to_owned))
                        .collect::<Option<Vec<_>>>()
                });
            let paths = paths.ok_or_else(|| {
                fail(
                    ErrorCode::InvalidJson,
                    "'drop' needs a list of paths in 'value'".into(),
                )
            })?;
            components::dropped(tree, id, paths);
        }
        (NodeType::Div | NodeType::Input, "keyDown") => {
            let keys = text()?;
            let keystroke = crate::keys::parse(&keys)
                .ok_or_else(|| fail(ErrorCode::InvalidJson, format!("Invalid key '{keys}'")))?;
            components::key_down(tree, id, &keystroke);
        }
        (NodeType::Checkbox | NodeType::Switch, "change") => {
            components::checkbox_clicked(tree, id, !checked);
        }
        // The controlled revert needs the window's InputState; only the report is simulated.
        (
            NodeType::Input | NodeType::Textarea | NodeType::NumberInput | NodeType::OtpInput,
            "change",
        ) => {
            components::input_edited(tree, id, &text()?);
        }
        // Choosing an option: an unknown one is an error, a disabled one or the current one does nothing.
        (NodeType::RadioGroup | NodeType::Select | NodeType::Tabs, "change") => {
            let value = text()?;
            let current = node.props.get("value").and_then(Value::as_str);
            let key = if node_type == NodeType::Tabs {
                "tabs"
            } else {
                "options"
            };
            match components::options(node, key)
                .into_iter()
                .find(|(option, ..)| *option == value)
            {
                None => {
                    return Err(fail(
                        ErrorCode::InvalidOperation,
                        format!("Node {id} has no option '{value}'"),
                    ));
                }
                Some((.., true)) => {}
                Some(_) if current == Some(value.as_str()) => {}
                Some(_) => {
                    components::value_chosen(tree, id, Value::String(value));
                }
            }
        }
        // Dragging to a value: it must be within `min` and `max`.
        (NodeType::Slider, "change") => {
            let value = sim.value.as_ref().and_then(Value::as_f64);
            let value = value.ok_or_else(|| {
                fail(
                    ErrorCode::InvalidJson,
                    "'change' on a Slider needs a number 'value'".into(),
                )
            })?;
            let bound = |key, default| {
                node.props
                    .get(key)
                    .and_then(Value::as_f64)
                    .unwrap_or(default)
            };
            if !(bound("min", 0.)..=bound("max", 100.)).contains(&value) {
                return Err(fail(
                    ErrorCode::InvalidOperation,
                    format!("{value} is outside the Slider's range"),
                ));
            }
            if node.props.get("value").and_then(Value::as_f64) != Some(value) {
                components::value_chosen(tree, id, components::number(value as f32));
            }
        }
        // Selecting a row (`null`: Escape clears the selection); the current one does nothing.
        // A double click selects the row first, as in the window.
        (NodeType::Table, "change" | "doubleClick") => {
            let rows = node
                .props
                .get("rows")
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            let row = match &sim.value {
                Some(Value::Null) | None if sim.event == "change" => None,
                value => match value
                    .as_ref()
                    .and_then(Value::as_u64)
                    .filter(|&ix| (ix as usize) < rows)
                {
                    Some(ix) => Some(ix as usize),
                    None => {
                        return Err(fail(
                            ErrorCode::InvalidOperation,
                            format!(
                                "Table {id} has no row {}",
                                value.clone().unwrap_or_default()
                            ),
                        ));
                    }
                },
            };
            let current = node
                .props
                .get("selected")
                .and_then(Value::as_u64)
                .map(|ix| ix as usize)
                .filter(|&ix| ix < rows);
            if row != current {
                components::table::row_selected(tree, id, row);
            }
            if let (Some(ix), "doubleClick") = (row, sim.event.as_str()) {
                components::table::row_double_clicked(tree, id, ix);
            }
        }
        // The sort button of a sortable column; the direction is given (the window cycles none → desc → asc).
        (NodeType::Table, "sort") => {
            let value = sim.value.as_ref();
            let column = value.and_then(|v| v.get("column")).and_then(Value::as_str);
            let direction = value
                .and_then(|v| v.get("direction"))
                .and_then(Value::as_str)
                .filter(|d| matches!(*d, "asc" | "desc" | "none"));
            let (Some(column), Some(direction)) = (column, direction) else {
                return Err(fail(
                    ErrorCode::InvalidJson,
                    "'sort' needs {\"column\", \"direction\": asc, desc or none} in 'value'".into(),
                ));
            };
            let sortable = node
                .props
                .get("columns")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .any(|spec| {
                    spec.get("key").and_then(Value::as_str) == Some(column)
                        && spec.get("sortable") == Some(&Value::Bool(true))
                });
            if !sortable {
                return Err(fail(
                    ErrorCode::InvalidOperation,
                    format!("Table {id} has no sortable column '{column}'"),
                ));
            }
            components::table::sorted(tree, id, column, direction);
        }
        // A star count: it must be within `max`; the current value does nothing.
        (NodeType::Rating, "change") => {
            let max = node
                .props
                .get("max")
                .and_then(Value::as_u64)
                .unwrap_or(5)
                .max(1);
            let value = sim
                .value
                .as_ref()
                .and_then(Value::as_u64)
                .filter(|value| (1..=max).contains(value));
            let value = value.ok_or_else(|| {
                fail(
                    ErrorCode::InvalidJson,
                    format!("'change' on a Rating needs a whole 'value' from 1 to {max}"),
                )
            })?;
            if node.props.get("value").and_then(Value::as_u64) != Some(value) {
                components::value_chosen(tree, id, Value::from(value));
            }
        }
        // A page: it must be within `total`; the current page does nothing.
        (NodeType::Pagination, "change") => {
            let total = node
                .props
                .get("total")
                .and_then(Value::as_u64)
                .unwrap_or(1)
                .max(1);
            let value = sim
                .value
                .as_ref()
                .and_then(Value::as_u64)
                .filter(|value| (1..=total).contains(value));
            let value = value.ok_or_else(|| {
                fail(
                    ErrorCode::InvalidJson,
                    format!("'change' on a Pagination needs a whole 'value' from 1 to {total}"),
                )
            })?;
            if node.props.get("page").and_then(Value::as_u64) != Some(value) {
                components::prop_chosen(tree, id, "page", Value::from(value));
            }
        }
        // A step: an unknown one is an error, a disabled one or the current one does nothing.
        (NodeType::Stepper, "change") => {
            let value = text()?;
            let current = node.props.get("value").and_then(Value::as_str);
            match components::options(node, "items")
                .into_iter()
                .find(|(step, ..)| *step == value)
            {
                None => {
                    return Err(fail(
                        ErrorCode::InvalidOperation,
                        format!("Node {id} has no step '{value}'"),
                    ));
                }
                Some((.., true)) => {}
                Some(_) if current == Some(value.as_str()) => {}
                Some(_) => {
                    components::value_chosen(tree, id, Value::String(value));
                }
            }
        }
        // A section of an Accordion: an unknown one is an error.
        (NodeType::Accordion, "change") => {
            let item = text()?;
            components::accordion::toggled(tree, id, &item)
                .map_err(|err| fail(ErrorCode::InvalidOperation, err))?;
        }
        // A day: it must be a date (`YYYY-MM-DD`); the current one does nothing.
        (NodeType::Calendar, "change") => {
            let value = text()?;
            if components::calendar::parse_date(&value).is_none() {
                return Err(fail(
                    ErrorCode::InvalidJson,
                    format!("Calendar {id} needs a 'value' like 2024-08-03, got '{value}'"),
                ));
            }
            if node.props.get("value").and_then(Value::as_str) != Some(value.as_str()) {
                components::value_chosen(tree, id, Value::String(value));
            }
        }
        // A tree item: an unknown id is an error, a disabled one or the current one does nothing.
        (NodeType::Tree, "change") => {
            let item = text()?;
            let items = components::tree::items(node);
            let Some(spec) = components::tree::find_spec(&items, &item) else {
                return Err(fail(
                    ErrorCode::InvalidOperation,
                    format!("Node {id} has no item '{item}'"),
                ));
            };
            if spec.disabled || node.props.get("value").and_then(Value::as_str) == Some(&item) {
            } else {
                components::tree::item_clicked(tree, id, &item);
            }
        }
        // Expanding or collapsing a folder; a leaf has nothing to expand.
        (NodeType::Tree, "expand" | "collapse") => {
            let item = text()?;
            let items = components::tree::items(node);
            let Some(spec) = components::tree::find_spec(&items, &item) else {
                return Err(fail(
                    ErrorCode::InvalidOperation,
                    format!("Node {id} has no item '{item}'"),
                ));
            };
            if !spec.children.is_empty() {
                let expanded = sim.event == "expand";
                components::tree::expansion_reported(tree, id, &item, expanded);
            }
        }
        // Selected source lines: `null`, or `{path, side, start, end}` of the patch.
        (NodeType::Diff, "change") => {
            let value = sim.value.clone().unwrap_or(Value::Null);
            if !value.is_null() {
                let text = |key| value.get(key).and_then(Value::as_str);
                let line = |key| value.get(key).and_then(Value::as_u64);
                let valid = text("path").is_some()
                    && matches!(text("side"), Some("original" | "modified"))
                    && matches!(text("endSide"), None | Some("original" | "modified"))
                    && line("start").is_some()
                    && line("end").is_some();
                if !valid {
                    return Err(fail(
                        ErrorCode::InvalidJson,
                        "'change' on a Diff needs null or {path, side, start, end, endSide?}"
                            .into(),
                    ));
                }
                let patch = node
                    .props
                    .get("patch")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let paths = gpui_kit::component::diff::DiffFile::parse(patch)
                    .map(|files| {
                        files
                            .iter()
                            .map(|file| file.path().to_string())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                if !paths.iter().any(|path| Some(path.as_str()) == text("path")) {
                    return Err(fail(
                        ErrorCode::InvalidOperation,
                        format!(
                            "The patch of node {id} has no file '{}'",
                            text("path").unwrap_or("")
                        ),
                    ));
                }
            }
            components::diff::selection_reported(tree, id, Some(value));
        }
        // A slide: it must be within the slides; the current one does nothing.
        (NodeType::Carousel, "change") => {
            let count = node.children.len() as u64;
            let index = sim
                .value
                .as_ref()
                .and_then(Value::as_u64)
                .filter(|index| *index < count);
            let index = index.ok_or_else(|| {
                fail(
                    ErrorCode::InvalidJson,
                    format!(
                        "'change' on a Carousel needs a whole 'value' from 0 to {}",
                        count.saturating_sub(1)
                    ),
                )
            })?;
            if node.props.get("value").and_then(Value::as_u64) != Some(index) {
                components::value_chosen(tree, id, Value::from(index));
            }
        }
        (NodeType::Input | NodeType::NumberInput, "submit") => {
            components::input_submitted(tree, id, text()?)
        }
        _ => unreachable!("checked by `known`"),
    }
    Ok(Outcome::Done)
}
