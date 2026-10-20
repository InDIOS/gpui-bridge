//! Layout components (PROTOCOL.md section 17): `Form` and `Field`, `GroupBox`, `DescriptionList`,
//! `StatusBar`, `Toolbar` and `Resizable`.

use gpui_kit::component::description_list::DescriptionList;
use gpui_kit::component::form::{Field, Form};
use gpui_kit::component::group_box::{GroupBox, GroupBoxVariants};
use gpui_kit::component::status_bar::StatusBar;
use gpui_kit::component::toolbar::Toolbar;
use gpui_kit::component::{h_resizable, resizable_panel, v_resizable};
use gpui_kit::*;
use serde_json::Value;

use super::{prop_bool, prop_str};
use crate::protocol::NodeId;
use crate::tree::Node;

/// A `Field`: a label, an optional description, and the children as its control.
pub fn field(node: &Node, children: Vec<AnyElement>) -> Field {
    let mut field = Field::new().required(prop_bool(node, "required"));
    if let Some(label) = prop_str(node, "label") {
        field = field.label(SharedString::from(label.to_owned()));
    }
    if let Some(description) = prop_str(node, "description") {
        field = field.description(SharedString::from(description.to_owned()));
    }
    field.children(children)
}

/// A `Form`: vertical (default) or horizontal, one `Field` per child. A child that is not a
/// `Field` goes in one without a label.
pub fn form(node: &Node, fields: Vec<Field>) -> Form {
    let form = match prop_str(node, "direction") {
        Some("horizontal") => Form::horizontal(),
        _ => Form::vertical(),
    };
    form.children(fields)
}

/// Props: `title`, `variant` (normal, fill, outline). Its children are the content.
pub fn group_box(id: NodeId, node: &Node) -> GroupBox {
    let mut group = GroupBox::new().id(("node", id));
    if let Some(title) = prop_str(node, "title") {
        group = group.title(SharedString::from(title.to_owned()));
    }
    match prop_str(node, "variant") {
        Some("fill") => group.fill(),
        Some("outline") => group.outline(),
        _ => group.normal(),
    }
}

/// An `items` entry: `(label, value, span)`.
type Entry = (String, String, usize);

/// The `items` of a DescriptionList.
pub fn entries(node: &Node) -> Vec<Entry> {
    let text = |item: &Value, key: &str| {
        item.get(key)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned()
    };
    node.props
        .get("items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|item| {
            (
                text(item, "label"),
                text(item, "value"),
                item.get("span").and_then(Value::as_u64).unwrap_or(1).max(1) as usize,
            )
        })
        .collect()
}

/// Checks an `items` list: objects with `label` and `value` strings and an optional `span`.
pub fn check_entries(value: &Value) -> Result<(), String> {
    let items = value.as_array().ok_or("a list of entries")?;
    for (ix, item) in items.iter().enumerate() {
        let ok = item.get("label").is_some_and(Value::is_string)
            && item.get("value").is_some_and(Value::is_string)
            && item
                .get("span")
                .is_none_or(|span| span.as_u64().is_some_and(|span| span >= 1) || span.is_null());
        if !ok {
            return Err(format!(
                "entry {ix} needs 'label' and 'value' strings (and an optional whole 'span' from 1)"
            ));
        }
    }
    Ok(())
}

/// Props: `items`, `columns` (default 1), `bordered`, `horizontal`.
pub fn description_list(node: &Node) -> DescriptionList {
    let mut list = if prop_bool(node, "horizontal") {
        DescriptionList::horizontal()
    } else {
        DescriptionList::new()
    };
    let columns = node
        .props
        .get("columns")
        .and_then(Value::as_u64)
        .unwrap_or(1)
        .max(1) as usize;
    list = list.columns(columns).bordered(prop_bool(node, "bordered"));
    for (label, value, span) in entries(node) {
        list = list.item(SharedString::from(label), SharedString::from(value), span);
    }
    list
}

/// Props: `left`, `right`.
pub fn status_bar(node: &Node) -> StatusBar {
    let mut bar = StatusBar::new();
    if let Some(left) = prop_str(node, "left") {
        bar = bar.left(SharedString::from(left.to_owned()));
    }
    if let Some(right) = prop_str(node, "right") {
        bar = bar.right(SharedString::from(right.to_owned()));
    }
    bar
}

/// A `Toolbar`; its children are its content.
pub fn toolbar(id: NodeId) -> Toolbar {
    Toolbar::new(("node", id))
}

/// Checks a `sizes` list: positive finite numbers, one per panel.
pub fn check_sizes(value: &Value) -> Result<(), String> {
    let sizes = value.as_array().ok_or("a list of sizes")?;
    for (ix, size) in sizes.iter().enumerate() {
        if !size
            .as_f64()
            .is_some_and(|size| size.is_finite() && size > 0.)
        {
            return Err(format!("size {ix} must be a number above 0"));
        }
    }
    Ok(())
}

/// The initial size of each panel of a `Resizable`, in pixels.
pub fn sizes(node: &Node) -> Vec<f32> {
    node.props
        .get("sizes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_f64)
        .map(|size| size as f32)
        .collect()
}

/// A `Resizable`: horizontal (default) or vertical, one panel per child. `sizes` gives each
/// panel its initial size; the panels without one share what is left.
pub fn resizable(id: NodeId, node: &Node, panels: Vec<AnyElement>) -> AnyElement {
    let sizes = sizes(node);
    let axis = match prop_str(node, "axis") {
        Some("vertical") => Axis::Vertical,
        _ => Axis::Horizontal,
    };
    let group = match axis {
        Axis::Vertical => v_resizable(("node", id)),
        _ => h_resizable(("node", id)),
    };
    let panels = panels
        .into_iter()
        .enumerate()
        .map(|(ix, panel)| match sizes.get(ix) {
            Some(size) => resizable_panel().size(px(*size)).flex_none().child(panel),
            None => resizable_panel().child(panel),
        });
    group.children(panels).into_any_element()
}
