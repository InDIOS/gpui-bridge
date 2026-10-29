//! The chat components of gpui-kit (PROTOCOL.md section 26): `Bubble`, `Message` (with its
//! `MessageHeader` and `MessageFooter` children), `Marker` and `Attachment`. All four are drawn
//! from props each frame and keep no state: an `Attachment`'s status is the client's.

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
