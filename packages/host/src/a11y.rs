//! Accessibility for every node (PROTOCOL.md section 16).
//!
//! GPUI reports to assistive technology (AccessKit) the elements that have both an id and a
//! role; a focused element without one makes it log a warning. Every node accepts an `a11y`
//! prop with those two and the ARIA-like state around them, so a client can name a `div` that
//! acts as a button, or a `Slider` that has no label of its own.
//!
//! The prop is applied to the `div` the host builds for `div` and `text`, and to a wrapper
//! around the element of any other type (a component cannot take an id or a role from outside).

use gpui_kit::*;
use serde_json::Value;

use crate::protocol::NodeId;
use crate::tree::Node;

/// The `a11y` prop of a node: what assistive technology reads, and what it may ask for.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct A11y {
    /// A stable identifier, visible outside the process (UIA `AutomationId` on Windows).
    pub id: Option<String>,
    pub role: Option<Role>,
    pub label: Option<String>,
    pub description: Option<String>,
    /// The keys that activate the element (`ctrl-s`), announced by a screen reader.
    pub keyshortcuts: Option<String>,
    pub checked: Option<Toggled>,
    pub selected: Option<bool>,
    pub expanded: Option<bool>,
    pub disabled: Option<bool>,
    pub value: Option<String>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub step: Option<f64>,
    pub level: Option<usize>,
    pub orientation: Option<Orientation>,
}

/// The `role` names a client may use, and the AccessKit role each one means.
///
/// The names follow ARIA (what a web developer would write), not AccessKit's variants.
const ROLES: &[(&str, Role)] = &[
    ("alert", Role::Alert),
    ("alertdialog", Role::AlertDialog),
    ("article", Role::Article),
    ("banner", Role::Banner),
    ("button", Role::Button),
    ("cell", Role::Cell),
    ("checkbox", Role::CheckBox),
    ("columnheader", Role::ColumnHeader),
    ("combobox", Role::ComboBox),
    ("complementary", Role::Complementary),
    ("contentinfo", Role::ContentInfo),
    ("definition", Role::Definition),
    ("dialog", Role::Dialog),
    ("document", Role::Document),
    ("form", Role::Form),
    ("grid", Role::Grid),
    ("gridcell", Role::GridCell),
    ("group", Role::Group),
    ("heading", Role::Heading),
    ("img", Role::Image),
    ("label", Role::Label),
    ("link", Role::Link),
    ("list", Role::List),
    ("listbox", Role::ListBox),
    ("listitem", Role::ListItem),
    ("log", Role::Log),
    ("main", Role::Main),
    ("menu", Role::Menu),
    ("menubar", Role::MenuBar),
    ("menuitem", Role::MenuItem),
    ("navigation", Role::Navigation),
    ("note", Role::Note),
    ("paragraph", Role::Paragraph),
    ("progressbar", Role::ProgressIndicator),
    ("radio", Role::RadioButton),
    ("radiogroup", Role::RadioGroup),
    ("region", Role::Region),
    ("row", Role::Row),
    ("rowheader", Role::RowHeader),
    ("scrollbar", Role::ScrollBar),
    ("searchbox", Role::SearchInput),
    ("separator", Role::Splitter),
    ("slider", Role::Slider),
    ("spinbutton", Role::SpinButton),
    ("status", Role::Status),
    ("switch", Role::Switch),
    ("tab", Role::Tab),
    ("table", Role::Table),
    ("tablist", Role::TabList),
    ("tabpanel", Role::TabPanel),
    ("textbox", Role::TextInput),
    ("time", Role::Time),
    ("toolbar", Role::Toolbar),
    ("tooltip", Role::Tooltip),
    ("tree", Role::Tree),
    ("treeitem", Role::TreeItem),
    ("window", Role::Window),
];

/// The role names a client may use (for error messages and the protocol document).
pub fn role_names() -> impl Iterator<Item = &'static str> {
    ROLES.iter().map(|(name, _)| *name)
}

/// Rejects an `a11y` prop that is not an object of the known keys with the right types.
pub fn check(value: &Value) -> Result<(), String> {
    let object = value.as_object().ok_or("an object")?;
    for (key, value) in object {
        let ok = match key.as_str() {
            "id" | "label" | "description" | "keyshortcuts" | "value" => value.is_string(),
            "role" => value.as_str().is_some_and(|name| role(name).is_some()),
            "checked" => value.is_boolean() || value.as_str() == Some("mixed"),
            "selected" | "expanded" | "disabled" => value.is_boolean(),
            "min" | "max" | "step" => value.as_f64().is_some_and(f64::is_finite),
            "level" => value.as_u64().is_some_and(|level| level >= 1),
            "orientation" => matches!(value.as_str(), Some("horizontal" | "vertical")),
            _ => {
                return Err(format!(
                    "no '{key}' (keys: id, role, label, description, keyshortcuts, checked, selected, expanded, disabled, value, min, max, step, level, orientation)"
                ));
            }
        };
        if !ok {
            return Err(match key.as_str() {
                "role" => format!(
                    "a known role ({}), got {value}",
                    role_names().collect::<Vec<_>>().join(", ")
                ),
                "checked" => format!("a boolean or \"mixed\" for 'checked', got {value}"),
                "orientation" => {
                    format!("\"horizontal\" or \"vertical\" for 'orientation', got {value}")
                }
                "level" => format!("a whole number from 1 for 'level', got {value}"),
                _ => format!("the right type for '{key}', got {value}"),
            });
        }
    }
    Ok(())
}

fn parse(value: &Value) -> Option<A11y> {
    let text = |key: &str| value.get(key).and_then(Value::as_str).map(str::to_owned);
    let boolean = |key: &str| value.get(key).and_then(Value::as_bool);
    let number = |key: &str| value.get(key).and_then(Value::as_f64);
    let a11y = A11y {
        id: text("id"),
        role: text("role").as_deref().and_then(role),
        label: text("label"),
        description: text("description"),
        keyshortcuts: text("keyshortcuts"),
        checked: match value.get("checked") {
            Some(Value::Bool(true)) => Some(Toggled::True),
            Some(Value::Bool(false)) => Some(Toggled::False),
            Some(Value::String(state)) if state == "mixed" => Some(Toggled::Mixed),
            _ => None,
        },
        selected: boolean("selected"),
        expanded: boolean("expanded"),
        disabled: boolean("disabled"),
        value: text("value"),
        min: number("min"),
        max: number("max"),
        step: number("step"),
        level: value
            .get("level")
            .and_then(Value::as_u64)
            .map(|level| level as usize),
        orientation: match text("orientation").as_deref() {
            Some("horizontal") => Some(Orientation::Horizontal),
            Some("vertical") => Some(Orientation::Vertical),
            _ => None,
        },
    };
    (a11y != A11y::default()).then_some(a11y)
}

fn role(name: &str) -> Option<Role> {
    ROLES
        .iter()
        .find(|(known, _)| *known == name)
        .map(|(_, role)| *role)
}

/// Applies `a11y` to `element`, which must not have an id yet: GPUI only reports elements that
/// have both an id and a role.
pub fn apply(a11y: &A11y, node: NodeId, element: Div) -> Stateful<Div> {
    decorate(a11y, element.id(("a11y", node)))
}

/// `apply` for an element the host already gave an id (a picture that takes clicks, for example).
pub fn apply_stateful(a11y: &A11y, element: Stateful<Div>) -> Stateful<Div> {
    decorate(a11y, element)
}

fn decorate(a11y: &A11y, mut element: Stateful<Div>) -> Stateful<Div> {
    // `Role::GenericContainer` is filtered out of the tree, so a role is only set when given.
    if let Some(role) = a11y.role {
        element = element.role(role);
    }
    if let Some(id) = &a11y.id {
        element = element.accessibility_id(id.clone());
    }
    if let Some(label) = &a11y.label {
        element = element.aria_label(label.clone());
    }
    if let Some(description) = &a11y.description {
        element = element.aria_description(description.clone());
    }
    if let Some(keys) = &a11y.keyshortcuts {
        element = element.aria_keyshortcuts(keys.clone());
    }
    if let Some(checked) = a11y.checked {
        element = element.aria_toggled(checked);
    }
    if let Some(selected) = a11y.selected {
        element = element.aria_selected(selected);
    }
    if let Some(expanded) = a11y.expanded {
        element = element.aria_expanded(expanded);
    }
    // A number with a range reads as a numeric value (a slider, a spin button); anything else
    // is the plain string value.
    // AccessKit names a `label` from its value, so without one the label is the value too.
    let value = match a11y.role {
        Some(Role::Label) => a11y.value.as_deref().or(a11y.label.as_deref()),
        _ => a11y.value.as_deref(),
    };
    match (value, numeric(a11y)) {
        (Some(_), Some(value)) => element = element.aria_numeric_value(value),
        (Some(value), None) => element = element.aria_value(value.to_owned()),
        _ => {}
    }
    if let Some(min) = a11y.min {
        element = element.aria_min_numeric_value(min);
    }
    if let Some(max) = a11y.max {
        element = element.aria_max_numeric_value(max);
    }
    if let Some(step) = a11y.step {
        element = element.aria_numeric_value_step(step);
    }
    if let Some(level) = a11y.level {
        element = element.aria_level(level);
    }
    if let Some(orientation) = a11y.orientation {
        element = element.aria_orientation(orientation);
    }
    element
}

/// The value as a number, when the node also gives a range or a step (so `"12"` is a slider's
/// position and not the text of a label).
fn numeric(a11y: &A11y) -> Option<f64> {
    let ranged = a11y.min.is_some() || a11y.max.is_some() || a11y.step.is_some();
    ranged
        .then(|| a11y.value.as_deref()?.parse::<f64>().ok())
        .flatten()
}

/// Wraps an element the host cannot give an id to (a component) so it still gets an
/// accessibility node. The wrapper is a row, the way a component behaves inside a parent.
pub fn wrap(a11y: &A11y, node: NodeId, child: AnyElement) -> AnyElement {
    apply(a11y, node, div().flex().child(child)).into_any_element()
}

/// The `a11y` prop of `node`, or `None` when it has none (or an empty one).
pub fn of(node: &Node) -> Option<A11y> {
    node.props
        .get("a11y")
        .filter(|value| value.is_object())
        .and_then(parse)
}

#[cfg(test)]
mod tests {
    // Explicit imports: `super::*` would bring GPUI's `test` macro, shadowing Rust's.
    use super::{A11y, ROLES, check, numeric, of, role};
    use crate::tree::Node;
    use gpui_kit::{Orientation, Role, Toggled};
    use serde_json::{Value, json};

    fn node(props: Value) -> Node {
        Node {
            node_type: crate::schema::NodeType::Div,
            props: props.as_object().cloned().unwrap_or_default(),
            events: Default::default(),
            children: vec![],
            parent: None,
        }
    }

    #[test]
    fn an_empty_prop_has_no_effect() {
        assert_eq!(of(&node(json!({ "a11y": {} }))), None);
        assert_eq!(of(&node(json!({ "class": "flex" }))), None);
    }

    #[test]
    fn reads_every_field() {
        let a11y = of(&node(json!({ "a11y": {
            "id": "save",
            "role": "button",
            "label": "Save",
            "description": "Writes the file",
            "keyshortcuts": "ctrl-s",
            "checked": "mixed",
            "selected": true,
            "expanded": false,
            "disabled": true,
            "value": "12",
            "min": 0,
            "max": 100,
            "step": 1,
            "level": 2,
            "orientation": "vertical",
        }})))
        .unwrap();
        assert_eq!(a11y.id.as_deref(), Some("save"));
        assert_eq!(a11y.role, Some(Role::Button));
        assert_eq!(a11y.checked, Some(Toggled::Mixed));
        assert_eq!(
            (a11y.selected, a11y.expanded, a11y.disabled),
            (Some(true), Some(false), Some(true))
        );
        assert_eq!(
            (a11y.min, a11y.max, a11y.step),
            (Some(0.), Some(100.), Some(1.))
        );
        assert_eq!(
            (a11y.level, a11y.orientation),
            (Some(2), Some(Orientation::Vertical))
        );
    }

    #[test]
    fn rejects_unknown_keys_and_bad_types() {
        for value in [
            json!({ "lable": "typo" }),
            json!({ "role": "widget" }),
            json!({ "role": 3 }),
            json!({ "checked": "yes" }),
            json!({ "selected": "yes" }),
            json!({ "level": 0 }),
            json!({ "orientation": "diagonal" }),
            json!({ "min": "0" }),
            json!([]),
            json!("label"),
        ] {
            assert!(check(&value).is_err(), "{value} must be rejected");
        }
        assert!(check(&json!({ "role": "button", "label": "OK" })).is_ok());
    }

    #[test]
    fn every_role_name_resolves() {
        for (name, expected) in ROLES {
            assert_eq!(role(name), Some(*expected));
        }
        assert_eq!(role("Button"), None);
    }

    #[test]
    fn only_a_ranged_value_is_a_number() {
        let plain = A11y {
            value: Some("12".into()),
            ..A11y::default()
        };
        assert_eq!(numeric(&plain), None);
        let ranged = A11y {
            value: Some("12".into()),
            min: Some(0.),
            ..A11y::default()
        };
        assert_eq!(numeric(&ranged), Some(12.));
    }
}
