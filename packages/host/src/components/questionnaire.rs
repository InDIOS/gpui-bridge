//! `Questionnaire` (PROTOCOL.md section 26): one question at a time from `items`, drawn, navigated
//! and checked by gpui-component (`required`, auto-advance, keys). One retained state per node,
//! rebuilt when `items` change. Each answer is reported with `change` and the whole set with
//! `submit`; the host also keeps the answers in the node, so `simulate` works without a window.

use std::collections::{HashMap, HashSet};

use gpui_kit::component::input::InputState;
use gpui_kit::component::questionnaire::{
    Questionnaire, QuestionnaireActions, QuestionnaireAnswer, QuestionnaireChoice,
    QuestionnaireChoiceDefinition, QuestionnaireChoices, QuestionnaireDescription,
    QuestionnaireError, QuestionnaireEvent, QuestionnaireInput, QuestionnaireInputDefinition,
    QuestionnaireItem, QuestionnaireItemDefinition, QuestionnaireItemStatus, QuestionnaireNext,
    QuestionnairePrevious, QuestionnaireProgress, QuestionnaireSkip, QuestionnaireState,
    QuestionnaireSubmit, QuestionnaireTitle,
};
use gpui_kit::*;
use serde::Deserialize;
use serde_json::{Value, json};

use super::emit;
use crate::Session;
use crate::protocol::NodeId;
use crate::schema::NodeType;
use crate::tree::{Node, Tree};

#[derive(Deserialize)]
struct Item {
    name: String,
    label: String,
    description: Option<String>,
    #[serde(default)]
    required: bool,
    #[serde(default)]
    multiple: bool,
    #[serde(default)]
    disabled: bool,
    #[serde(default)]
    input: bool,
    #[serde(default)]
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    value: String,
    label: String,
    description: Option<String>,
    #[serde(default)]
    disabled: bool,
    #[serde(default)]
    selected: bool,
}

/// `items`, checked: what the crate rejects (a repeated name or value, two picks in a single-
/// choice question) is `invalid_props` here.
fn parse(value: &Value) -> Result<Vec<Item>, String> {
    let items: Vec<Item> = serde_json::from_value(value.clone()).map_err(|err| err.to_string())?;
    let mut names = HashSet::new();
    for item in &items {
        if item.name.is_empty() || !names.insert(&item.name) {
            return Err(format!(
                "item names must be unique and not empty ('{}')",
                item.name
            ));
        }
        let mut values = HashSet::new();
        if let Some(choice) = item.choices.iter().find(|c| !values.insert(&c.value)) {
            return Err(format!(
                "item '{}' repeats choice '{}'",
                item.name, choice.value
            ));
        }
        if !item.multiple && item.choices.iter().filter(|c| c.selected).count() > 1 {
            return Err(format!(
                "item '{}' takes one choice but selects several",
                item.name
            ));
        }
    }
    Ok(items)
}

pub fn check_items(value: &Value) -> Result<(), String> {
    parse(value).map(|_| ())
}

fn items(node: &Node) -> Vec<Item> {
    node.props
        .get("items")
        .and_then(|items| parse(items).ok())
        .unwrap_or_default()
}

/// The answer as the wire carries it.
fn answer(name: &str, status: &str, choices: &[impl AsRef<str>], text: Option<&str>) -> Value {
    let choices: Vec<&str> = choices.iter().map(AsRef::as_ref).collect();
    json!({"name": name, "status": status, "choices": choices, "text": text})
}

fn status(status: QuestionnaireItemStatus) -> &'static str {
    match status {
        QuestionnaireItemStatus::Answered => "answered",
        QuestionnaireItemStatus::Skipped => "skipped",
        _ => "unanswered",
    }
}

fn from_crate(name: &str, state: QuestionnaireItemStatus, given: &QuestionnaireAnswer) -> Value {
    answer(
        name,
        status(state),
        given.choices(),
        given.freeform().map(|text| text.as_ref()),
    )
}

/// Keeps `answer` in the node's `answers` (by item name) and reports it with `change`.
fn answered(tree: &mut Tree, id: NodeId, answer: Value) {
    let Some(node) = tree.node(id) else { return };
    let mut answers = node
        .props
        .get("answers")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    answers.insert(
        answer["name"].as_str().unwrap_or_default().into(),
        answer.clone(),
    );
    tree.set_host_prop(id, "answers", Value::Object(answers));
    emit(tree, id, "change", Some(answer), None);
}

pub struct QuestionnaireSlot {
    state: Entity<QuestionnaireState>,
    /// What the state was built from: it is rebuilt when it changes.
    items: Value,
    _events: Subscription,
}

/// Each Questionnaire among `members` with its `items`.
pub fn snapshot(tree: &Tree, members: &HashSet<NodeId>) -> Vec<(NodeId, Value)> {
    tree.nodes()
        .filter(|(id, node)| node.node_type == NodeType::Questionnaire && members.contains(id))
        .map(|(id, node)| (id, node.props.get("items").cloned().unwrap_or_default()))
        .collect()
}

pub fn sync<V: 'static>(
    slots: &mut HashMap<NodeId, QuestionnaireSlot>,
    found: Vec<(NodeId, Value)>,
    session: &Entity<Session>,
    window: &mut Window,
    cx: &mut Context<V>,
) {
    slots.retain(|id, _| found.iter().any(|(node, _)| node == id));
    for (id, value) in found {
        if slots.get(&id).is_some_and(|slot| slot.items == value) {
            continue;
        }
        let definitions = parse(&value)
            .unwrap_or_default()
            .into_iter()
            .map(|item| definition(item, window, cx))
            .collect();
        // `parse` already rejects what the crate's schema check does; this is only a fallback.
        let state = cx.new(|cx| match QuestionnaireState::new(definitions, cx) {
            Ok(state) => state,
            Err(err) => {
                log::error!("Questionnaire {id}: {err:?}");
                QuestionnaireState::new(vec![], cx).expect("no items is a valid schema")
            }
        });
        let session = session.clone();
        let _events = cx.subscribe_in(
            &state,
            window,
            move |_, _, event: &QuestionnaireEvent, _, cx| {
                session.update(cx, |session, _| match event {
                    QuestionnaireEvent::AnswerChanged(change) => {
                        let answer = from_crate(change.item(), change.status(), change.answer());
                        answered(&mut session.tree, id, answer);
                    }
                    QuestionnaireEvent::Submit(submission) => {
                        let answers = submission
                            .items()
                            .iter()
                            .map(|item| from_crate(item.name(), item.status(), item.answer()))
                            .collect();
                        emit(
                            &session.tree,
                            id,
                            "submit",
                            Some(Value::Array(answers)),
                            None,
                        );
                    }
                    _ => {}
                });
            },
        );
        slots.insert(
            id,
            QuestionnaireSlot {
                state,
                items: value,
                _events,
            },
        );
    }
}

fn definition<V: 'static>(
    item: Item,
    window: &mut Window,
    cx: &mut Context<V>,
) -> QuestionnaireItemDefinition {
    let mut definition = QuestionnaireItemDefinition::new(item.name, item.label.clone())
        .with_required(item.required)
        .with_multiple(item.multiple)
        .with_disabled(item.disabled)
        .with_choices(item.choices.into_iter().map(|choice| {
            let mut definition = QuestionnaireChoiceDefinition::new(choice.value, choice.label)
                .with_disabled(choice.disabled)
                .with_default_selected(choice.selected);
            if let Some(description) = choice.description {
                definition = definition.with_description(description);
            }
            definition
        }));
    if let Some(description) = item.description {
        definition = definition.with_description(description);
    }
    if item.input {
        let input = cx.new(|cx| InputState::new(window, cx));
        definition = definition.with_input(QuestionnaireInputDefinition::new(input, item.label));
    }
    definition
}

/// The crate's own parts, in its usual order: progress, then each question (title, description,
/// choices, free text, error), then the navigation buttons. Each part shows only while it applies.
pub fn build(node: &Node, slot: &QuestionnaireSlot) -> Questionnaire {
    let state = &slot.state;
    let questions = items(node).into_iter().map(|item| {
        let name = SharedString::from(item.name);
        let mut question = QuestionnaireItem::new(state, name.clone())
            .child(QuestionnaireTitle::new(state, name.clone()))
            .child(QuestionnaireDescription::new(state, name.clone()));
        if !item.choices.is_empty() {
            let choices = item
                .choices
                .into_iter()
                .map(|choice| QuestionnaireChoice::new(state, name.clone(), choice.value));
            question =
                question.child(QuestionnaireChoices::new(state, name.clone()).children(choices));
        }
        if item.input {
            question = question.child(QuestionnaireInput::new(state, name.clone()));
        }
        question.child(QuestionnaireError::new(state, name))
    });
    Questionnaire::new(state)
        .child(QuestionnaireProgress::new(state))
        .children(questions)
        .child(
            QuestionnaireActions::new(state)
                .child(QuestionnairePrevious::new(state))
                .child(QuestionnaireSkip::new(state))
                .child(QuestionnaireNext::new(state))
                .child(QuestionnaireSubmit::new(state)),
        )
}

/// The answers the node holds now, in item order: the reported ones, else each item's
/// `selected` choices.
fn current(node: &Node, items: &[Item]) -> Vec<Value> {
    let stored = node.props.get("answers").and_then(Value::as_object);
    items
        .iter()
        .filter(|item| !item.disabled)
        .map(|item| {
            let given = stored.and_then(|answers| answers.get(&item.name)).cloned();
            given.unwrap_or_else(|| {
                let picked: Vec<&str> = item
                    .choices
                    .iter()
                    .filter(|c| c.selected && !c.disabled)
                    .map(|c| c.value.as_str())
                    .collect();
                let state = if picked.is_empty() {
                    "unanswered"
                } else {
                    "answered"
                };
                answer(&item.name, state, &picked, None)
            })
        })
        .collect()
}

/// `simulate change` with `{name, choices?, text?}`: an answer the user could give.
pub fn simulate_change(tree: &mut Tree, id: NodeId, value: &Value) -> Result<(), String> {
    let node = tree.node(id).ok_or("no node")?;
    let name = value
        .get("name")
        .and_then(Value::as_str)
        .ok_or("'value' needs a 'name'")?;
    let items = items(node);
    let item = items
        .iter()
        .find(|item| item.name == name && !item.disabled)
        .ok_or_else(|| format!("no enabled item '{name}'"))?;
    let choices: Vec<&str> = value
        .get("choices")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|choice| choice.as_str().ok_or("'choices' must be strings"))
        .collect::<Result<_, _>>()?;
    let offered = |choice: &&str| {
        item.choices
            .iter()
            .any(|c| c.value == *choice && !c.disabled)
    };
    if !choices.iter().all(offered) || (!item.multiple && choices.len() > 1) {
        return Err(format!("item '{name}' cannot take {choices:?}"));
    }
    let text = value.get("text").and_then(Value::as_str);
    if text.is_some() && !item.input {
        return Err(format!("item '{name}' has no free text"));
    }
    let filled = !choices.is_empty() || text.is_some_and(|text| !text.trim().is_empty());
    let state = if filled { "answered" } else { "unanswered" };
    answered(tree, id, answer(name, state, &choices, text));
    Ok(())
}

/// `simulate submit`: sent only when every enabled item is answered, as the crate requires (it
/// moves to the first one missing instead). Skipping only exists in a window.
pub fn simulate_submit(tree: &Tree, id: NodeId) {
    let Some(node) = tree.node(id) else { return };
    let answers = current(node, &items(node));
    if answers
        .iter()
        .all(|answer| answer["status"] != "unanswered")
    {
        emit(tree, id, "submit", Some(Value::Array(answers)), None);
    }
}
