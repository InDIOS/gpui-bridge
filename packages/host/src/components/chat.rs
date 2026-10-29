//! The chat components of gpui-kit (PROTOCOL.md section 26): `Bubble`, `Message` (with its
//! `MessageHeader` and `MessageFooter` children), `Marker`, `Attachment` and `MessageScroller`.
//! They are drawn from props each frame; only a `MessageScroller` keeps state, its scroll position
//! (an `Attachment`'s status is the client's).

use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::path::PathBuf;

use gpui_kit::component::Icon;
use gpui_kit::component::attachment::{
    Attachment, AttachmentActions, AttachmentContent, AttachmentDescription, AttachmentMedia,
    AttachmentStatus, AttachmentTitle,
};
use gpui_kit::component::avatar::Avatar;
use gpui_kit::component::bubble::{Bubble, BubbleVariant};
use gpui_kit::component::marker::{
    Marker, MarkerAlignment, MarkerContent, MarkerIcon, MarkerLoadingStyle, MarkerVariant,
};
use gpui_kit::component::message::{
    Message, MessageAlignment, MessageContent, MessageFooter, MessageHeader,
};
use gpui_kit::component::message_scroller::{MessageScroller, MessageScrollerState};
use gpui_kit::*;
use serde_json::Value;

use super::basic::icon;
use super::{emit, icon_path, prop_bool, prop_f32, prop_str};
use crate::Session;
use crate::protocol::NodeId;
use crate::tree::{Node, Tree};

fn alignment(node: &Node) -> Option<MessageAlignment> {
    match prop_str(node, "alignment")? {
        "end" => Some(MessageAlignment::End),
        _ => Some(MessageAlignment::Start),
    }
}

/// Whether `node` is a `Bubble` with no surface, which drops the inset of its message's header
/// and footer (the crate does the same for a bubble it is handed as such).
pub fn is_ghost(node: &Node) -> bool {
    prop_str(node, "variant") == Some("ghost")
}

/// Props: `variant`, `alignment` (unset, it follows its `Message`). The children are the content.
pub fn bubble(node: &Node) -> Bubble {
    let variant = match prop_str(node, "variant") {
        Some("secondary") => BubbleVariant::Secondary,
        Some("muted") => BubbleVariant::Muted,
        Some("tinted") => BubbleVariant::Tinted,
        Some("outline") => BubbleVariant::Outline,
        Some("ghost") => BubbleVariant::Ghost,
        Some("destructive") => BubbleVariant::Destructive,
        _ => BubbleVariant::Filled,
    };
    let bubble = Bubble::new().with_variant(variant);
    match alignment(node) {
        Some(alignment) => bubble.alignment(alignment),
        None => bubble,
    }
}

/// Props: `alignment`, `avatarName`/`avatarSrc` (an `Avatar`'s `name`/`src`). The header and
/// footer come from `MessageHeader`/`MessageFooter` children; `content` is every other child.
pub fn message(
    node: &Node,
    header: Option<MessageHeader>,
    content: Vec<AnyElement>,
    footer: Option<MessageFooter>,
    ghost: bool,
) -> Message {
    let mut message = Message::new()
        .alignment(alignment(node).unwrap_or(MessageAlignment::Start))
        .content(MessageContent::new().children(content));
    let name = prop_str(node, "avatarName");
    let src = prop_str(node, "avatarSrc").filter(|src| !src.is_empty());
    if name.is_some() || src.is_some() {
        let mut avatar = Avatar::new();
        if let Some(name) = name {
            avatar = avatar.name(name.to_owned());
        }
        if let Some(src) = src {
            avatar = avatar.src(ImageSource::from(PathBuf::from(src)));
        }
        message = message.avatar(avatar);
    }
    if let Some(header) = header {
        message = message.header(header.content_inset(!ghost));
    }
    if let Some(footer) = footer {
        message = message.footer(footer.content_inset(!ghost));
    }
    message
}

/// Props: `content` (text, which takes the shimmer while loading), `icon`, `variant`,
/// `alignment` (unset, a separator centers), `loading`, `loadingStyle`. The children follow the
/// text inside the content slot.
pub fn marker(node: &Node, children: Vec<AnyElement>) -> Marker {
    let variant = match prop_str(node, "variant") {
        Some("separator") => MarkerVariant::Separator,
        Some("border") => MarkerVariant::Border,
        _ => MarkerVariant::Plain,
    };
    let loading_style = match prop_str(node, "loadingStyle") {
        Some("shimmer") => MarkerLoadingStyle::Shimmer,
        _ => MarkerLoadingStyle::Spinner,
    };
    let mut marker = Marker::new()
        .with_variant(variant)
        .loading(prop_bool(node, "loading"))
        .with_loading_style(loading_style);
    match prop_str(node, "alignment") {
        Some("start") => marker = marker.alignment(MarkerAlignment::Start),
        Some("center") => marker = marker.alignment(MarkerAlignment::Center),
        Some("end") => marker = marker.alignment(MarkerAlignment::End),
        _ => {}
    }
    if let Some(icon) = icon(node, "icon") {
        marker = marker.icon(MarkerIcon::new().child(icon));
    }
    let text = prop_str(node, "content");
    if text.is_none() && children.is_empty() {
        return marker;
    }
    let mut content = MarkerContent::new();
    if let Some(text) = text {
        content = content.text(text.to_owned());
    }
    marker.content(content.children(children))
}

/// Props: `status`, `progress`, `title`, `description`, `tooltip`, `axis` and the picture
/// (`image`, from `src`/`data` as in an `Image`; without one the media shows a file icon). The
/// children are the actions. Each built-in control exists only while its event is subscribed:
/// `click` on the card, `remove` on the corner button and `retry` while `failed`.
pub fn attachment(
    id: NodeId,
    node: &Node,
    image: Option<PathBuf>,
    actions: Vec<AnyElement>,
    session: &Entity<Session>,
) -> Attachment {
    let mut attachment = Attachment::new()
        .id(("node", id))
        .status(match prop_str(node, "status") {
            Some("pending") => AttachmentStatus::Pending,
            Some("uploading") => AttachmentStatus::Uploading,
            Some("processing") => AttachmentStatus::Processing,
            Some("failed") => AttachmentStatus::Failed,
            _ => AttachmentStatus::Complete,
        })
        .axis(match prop_str(node, "axis") {
            Some("vertical") => Axis::Vertical,
            _ => Axis::Horizontal,
        });
    if let Some(progress) = prop_f32(node, "progress") {
        attachment = attachment.progress(progress);
    }
    if let Some(tooltip) = prop_str(node, "tooltip") {
        attachment = attachment.tooltip(tooltip.to_owned());
    }
    attachment = attachment.media(match image {
        Some(path) => AttachmentMedia::new().src(path),
        None => AttachmentMedia::new().child(Icon::default().path(icon_path("file"))),
    });
    let title = prop_str(node, "title");
    let description = prop_str(node, "description");
    if title.is_some() || description.is_some() {
        let mut content = AttachmentContent::new();
        if let Some(title) = title {
            content = content.title(AttachmentTitle::new(title.to_owned()));
        }
        if let Some(description) = description {
            content = content.description(AttachmentDescription::new(description.to_owned()));
        }
        attachment = attachment.content(content);
    }
    if !actions.is_empty() {
        attachment = attachment.actions(AttachmentActions::new().children(actions));
    }
    for event in ["click", "remove", "retry"] {
        if !node.events.contains(event) {
            continue;
        }
        let session = session.clone();
        let handler = move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
            attachment_action(&session.read(cx).tree, id, event)
        };
        attachment = match event {
            "click" => attachment.on_click(handler),
            "remove" => attachment.on_remove(handler),
            _ => attachment.on_retry(handler),
        };
    }
    attachment
}

/// `click`, `remove` or `retry` on an `Attachment`. A `retry` only exists while `failed`.
pub fn attachment_action(tree: &Tree, id: NodeId, event: &str) {
    let failed = tree
        .node(id)
        .and_then(|node| node.props.get("status"))
        .and_then(Value::as_str)
        == Some("failed");
    if event != "retry" || failed {
        emit(tree, id, event, None, None);
    }
}

/// One per `MessageScroller`: its list state and the rows it was last given.
pub type ScrollerSlot = (Entity<MessageScrollerState>, Vec<NodeId>);

/// Tells each scroller which of its rows changed, so the list keeps the reading position (and
/// stays on the latest message while it is there). Rows are compared by id.
pub fn sync_scrollers<V: 'static>(
    slots: &mut HashMap<NodeId, ScrollerSlot>,
    found: Vec<(NodeId, Vec<NodeId>)>,
    cx: &mut Context<V>,
) {
    let ids: HashSet<NodeId> = found.iter().map(|(id, _)| *id).collect();
    slots.retain(|id, _| ids.contains(id));
    for (id, rows) in found {
        match slots.get_mut(&id) {
            None => {
                let state = cx.new(|cx| MessageScrollerState::new(rows.len(), cx));
                slots.insert(id, (state, rows));
            }
            Some((state, old)) if *old != rows => {
                let (range, count) = changed(old, &rows);
                state.update(cx, |state, cx| state.splice(range, count, cx));
                *old = rows;
            }
            Some(_) => {}
        }
    }
}

/// The rows of `old` that `new` replaces and how many take their place: what is left between
/// the common start and the common end. An append or a prepend is one splice at an edge.
fn changed(old: &[NodeId], new: &[NodeId]) -> (Range<usize>, usize) {
    let start = old.iter().zip(new).take_while(|(a, b)| a == b).count();
    let end = old[start..]
        .iter()
        .rev()
        .zip(new[start..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    (start..old.len() - end, new.len() - start - end)
}

/// Props: `scrollbar`, `jumpButton` (both on by default), `jumpLabel` (the jump button's tooltip
/// and name). `row` draws the child at an index; only the rows in view are drawn.
pub fn message_scroller(
    id: NodeId,
    node: &Node,
    state: &Entity<MessageScrollerState>,
    row: impl FnMut(usize, &mut Window, &mut App) -> AnyElement + 'static,
) -> MessageScroller {
    let on = |key| node.props.get(key).and_then(Value::as_bool).unwrap_or(true);
    let scroller = MessageScroller::new(("node", id), state.clone(), row)
        .scrollbar(on("scrollbar"))
        .jump_button(on("jumpButton"));
    match prop_str(node, "jumpLabel") {
        Some(label) => scroller.with_jump_button_label(label.to_owned()),
        None => scroller,
    }
}

#[cfg(test)]
mod tests {
    use super::changed;

    #[test]
    fn a_row_change_is_one_splice() {
        // Append, prepend, a removal in the middle, one row replaced, all replaced.
        assert_eq!(changed(&[1, 2], &[1, 2, 3, 4]), (2..2, 2));
        assert_eq!(changed(&[3, 4], &[1, 2, 3, 4]), (0..0, 2));
        assert_eq!(changed(&[1, 2, 3], &[1, 3]), (1..2, 0));
        assert_eq!(changed(&[1, 2, 3], &[1, 9, 3]), (1..2, 1));
        assert_eq!(changed(&[1, 2], &[3]), (0..2, 1));
        // A repeated id at the seam is not counted twice.
        assert_eq!(changed(&[1, 1], &[1]), (1..2, 0));
    }
}
