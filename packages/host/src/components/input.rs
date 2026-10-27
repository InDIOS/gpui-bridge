//! Text inputs keep their content in the host (PROTOCOL.md section 7): one retained state per
//! `Input` (single line) or `Textarea` (multi-line) node, synced after each commit.

use std::collections::{HashMap, HashSet};

use gpui_kit::base::input::{InputBaseState, InputModeKind};
use gpui_kit::component::Disableable;
use gpui_kit::component::input::{
    Input, InputEvent, InputGroup, InputGroupAddon, InputGroupAddonAlignment, InputMode,
    InputState, NumberInput, Textarea, TextareaMode, TextareaState,
};
use gpui_kit::*;

use super::{basic, input_edited, input_submitted, prop_bool, prop_str};
use crate::Session;
use crate::a11y;
use crate::protocol::NodeId;
use crate::schema::NodeType;
use crate::tree::{Node, Tree};

/// How the state of a text node of mode `M` is created.
pub trait TextKind: InputModeKind {
    /// Single line: Enter submits (instead of adding a line) and `controlled` applies.
    const SINGLE_LINE: bool;
    fn new_state(
        window: &mut Window,
        cx: &mut Context<InputBaseState<Self>>,
    ) -> InputBaseState<Self>;
}

impl TextKind for InputMode {
    const SINGLE_LINE: bool = true;
    fn new_state(window: &mut Window, cx: &mut Context<InputState>) -> InputState {
        InputState::new(window, cx)
    }
}

impl TextKind for TextareaMode {
    const SINGLE_LINE: bool = false;
    fn new_state(window: &mut Window, cx: &mut Context<TextareaState>) -> TextareaState {
        TextareaState::new(window, cx)
    }
}

pub struct InputSlot<M: TextKind> {
    state: Entity<InputBaseState<M>>,
    placeholder: String,
    _changes: Subscription,
}

impl<M: TextKind> InputSlot<M> {
    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        self.state.update(cx, |state, cx| state.focus(window, cx));
    }
}

/// What `sync` needs from the tree, copied out so the tree borrow ends before
/// entities are created or updated.
pub struct InputSnapshot {
    /// Every text node of the kind, with its placeholder.
    inputs: Vec<(NodeId, String)>,
    /// Imposed values to write, in order.
    values: Vec<(NodeId, String)>,
}

/// The text nodes of type `node_type` among `members` (one window's nodes) and the imposed values for them.
pub fn snapshot(
    tree: &Tree,
    node_type: NodeType,
    members: &HashSet<NodeId>,
    value_writes: &[NodeId],
) -> InputSnapshot {
    let text = |node: &Node, key| prop_str(node, key).unwrap_or("").to_owned();
    InputSnapshot {
        inputs: tree
            .nodes()
            .filter(|(id, node)| node.node_type == node_type && members.contains(id))
            .map(|(id, node)| (id, text(node, "placeholder")))
            .collect(),
        values: value_writes
            .iter()
            .filter(|id| members.contains(id))
            .filter_map(|&id| {
                tree.node(id)
                    .filter(|node| node.node_type == node_type)
                    .map(|node| (id, text(node, "value")))
            })
            .collect(),
    }
}

/// Creates states for new text nodes, drops removed ones, and applies imposed
/// values and placeholder changes.
pub fn sync<M: TextKind, V: 'static>(
    slots: &mut HashMap<NodeId, InputSlot<M>>,
    snapshot: InputSnapshot,
    session: &Entity<Session>,
    window: &mut Window,
    cx: &mut Context<V>,
) {
    slots.retain(|id, _| snapshot.inputs.iter().any(|(input, _)| input == id));

    for (id, placeholder) in snapshot.inputs {
        let slot = slots.entry(id).or_insert_with(|| {
            let state = cx.new(|cx| M::new_state(window, cx));
            let session = session.clone();
            let _changes = cx.subscribe_in(
                &state,
                window,
                move |_, state, event: &InputEvent, window, cx| match event {
                    InputEvent::Change => on_user_edit(id, state, &session, window, cx),
                    InputEvent::PressEnter { .. } if M::SINGLE_LINE => {
                        let value = state.read(cx).value().to_string();
                        input_submitted(&session.read(cx).tree, id, value);
                    }
                    _ => {}
                },
            );
            InputSlot {
                state,
                placeholder: String::new(),
                _changes,
            }
        });
        if slot.placeholder != placeholder {
            slot.placeholder = placeholder.clone();
            slot.state.update(cx, |state, cx| {
                state.set_placeholder(placeholder, window, cx)
            });
        }
    }

    for (id, value) in snapshot.values {
        // Skipping an identical value keeps the caret where the user left it.
        if let Some(slot) = slots
            .get(&id)
            .filter(|slot| slot.state.read(cx).value() != value)
        {
            slot.state
                .update(cx, |state, cx| state.set_value(value, window, cx));
        }
    }
}

/// Reports the edit; when controlled, reverts to the last client-imposed value
/// until the client accepts the edit with `setProps`.
fn on_user_edit<M: TextKind>(
    id: NodeId,
    state: &Entity<InputBaseState<M>>,
    session: &Entity<Session>,
    window: &mut Window,
    cx: &mut App,
) {
    let text = state.read(cx).value().to_string();
    let revert = input_edited(&session.read(cx).tree, id, &text).filter(|_| M::SINGLE_LINE);
    if let Some(accepted) = revert {
        state.update(cx, |state, cx| state.set_value(accepted, window, cx));
    }
}

/// Props: `value` (applied by `sync`), `placeholder`, `disabled`, `controlled`.
pub fn build(node: &Node, slot: &InputSlot<InputMode>) -> Input {
    Input::new(&slot.state).disabled(prop_bool(node, "disabled"))
}

/// Props like `Input`, plus `prefix`/`prefixIcon` and `suffix`/`suffixIcon` (text and an icon on
/// either side of the frame), `readonly` and `invalid`. Events: `change`, `submit`.
/// `a11y.label` names the group itself, so the host does not wrap it (see `node_raw`).
pub fn build_group(id: NodeId, node: &Node, slot: &InputSlot<InputMode>) -> InputGroup {
    let mut group = InputGroup::new(("node", id))
        .input(build(node, slot))
        .disabled(prop_bool(node, "disabled"))
        .readonly(prop_bool(node, "readonly"))
        .invalid(prop_bool(node, "invalid"));
    if let Some(label) = a11y::of(node).and_then(|spec| spec.label) {
        group = group.aria_label(label);
    }
    // A side reads icon then text on the left, text then icon on the right; with neither it adds
    // no addon at all.
    for (text_key, icon_key, align, icon_first) in [
        (
            "prefix",
            "prefixIcon",
            InputGroupAddonAlignment::InlineStart,
            true,
        ),
        (
            "suffix",
            "suffixIcon",
            InputGroupAddonAlignment::InlineEnd,
            false,
        ),
    ] {
        let text = prop_str(node, text_key)
            .filter(|text| !text.is_empty())
            .map(|text| SharedString::from(text.to_owned()).into_any_element());
        let icon = basic::icon(node, icon_key).map(IntoElement::into_any_element);
        let children: Vec<AnyElement> = if icon_first {
            [icon, text].into_iter().flatten().collect()
        } else {
            [text, icon].into_iter().flatten().collect()
        };
        if children.is_empty() {
            continue;
        }
        group = group.addon(
            InputGroupAddon::new((text_key, id))
                .align(align)
                .children(children),
        );
    }
    group
}

/// Props like `Input`; it adds the increment and decrement buttons of gpui-component.
pub fn build_number(node: &Node, slot: &InputSlot<InputMode>) -> NumberInput {
    let input = NumberInput::new(&slot.state).disabled(prop_bool(node, "disabled"));
    match prop_str(node, "placeholder") {
        Some(placeholder) => input.placeholder(placeholder.to_owned()),
        None => input,
    }
}

/// Props: `value` (applied by `sync`), `placeholder`, `disabled`. Always synced: a controlled
/// revert would move the caret to the end on every edit.
pub fn build_textarea(node: &Node, slot: &InputSlot<TextareaMode>) -> Textarea {
    Textarea::new(&slot.state).disabled(prop_bool(node, "disabled"))
}
