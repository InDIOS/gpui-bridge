//! Basic components of gpui-kit (PROTOCOL.md section 17): alert, badge, tag, kbd, progress,
//! spinner, skeleton, rating, pagination, collapsible, avatar and stepper.
//!
//! They are stateless except where a user choice is reported (`Rating`, `Pagination`, `Stepper`),
//! which work like the other choices: the host keeps the value unless the node is `controlled`.

use std::time::Duration;

use gpui_kit::component::Icon;
use gpui_kit::component::alert::{Alert, AlertVariant};
use gpui_kit::component::avatar::Avatar;
use gpui_kit::component::badge::Badge;
use gpui_kit::component::collapsible::Collapsible;
use gpui_kit::component::empty::{
    Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyMediaVariant, EmptyTitle,
};
use gpui_kit::component::kbd::Kbd;
use gpui_kit::component::label::Label;
use gpui_kit::component::pagination::Pagination;
use gpui_kit::component::progress::Progress;
use gpui_kit::component::rating::Rating;
use gpui_kit::component::shimmer::ShimmerText;
use gpui_kit::component::skeleton::Skeleton;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::stepper::{Stepper, StepperItem};
use gpui_kit::component::tag::Tag;
use gpui_kit::*;
use serde_json::Value;

use super::{
    alert_closed, icon_path, options, prop_bool, prop_chosen, prop_f32, prop_str, value_chosen,
};
use crate::Session;
use crate::protocol::NodeId;
use crate::tree::Node;

/// An icon by its bundled name.
pub fn icon(node: &Node, key: &str) -> Option<Icon> {
    prop_str(node, key).map(|name| Icon::default().path(icon_path(name)))
}

/// Props: `message`, `title`, `variant` (default, info, success, warning, error), `banner`,
/// `closable`. Events: `close`.
pub fn alert(id: NodeId, node: &Node, session: &Entity<Session>) -> Alert {
    let message = prop_str(node, "message").unwrap_or("").to_owned();
    let mut alert = Alert::new(("node", id), message);
    if let Some(title) = prop_str(node, "title") {
        alert = alert.title(title.to_owned());
    }
    alert = match prop_str(node, "variant") {
        Some("info") => alert.with_variant(AlertVariant::Info),
        Some("success") => alert.with_variant(AlertVariant::Success),
        Some("warning") => alert.with_variant(AlertVariant::Warning),
        Some("error") => alert.with_variant(AlertVariant::Error),
        _ => alert,
    };
    if prop_bool(node, "banner") {
        alert = alert.banner();
    }
    if prop_bool(node, "closable") {
        let session = session.clone();
        alert = alert.on_close(move |_, _, cx| alert_closed(&session.read(cx).tree, id));
    }
    alert
}

/// Props: `count`, `max`, `dot`, `icon`. Its children are shown under the badge.
pub fn badge(node: &Node) -> Badge {
    let mut badge = Badge::new();
    if prop_bool(node, "dot") {
        badge = badge.dot();
    } else if let Some(icon) = icon(node, "icon") {
        badge = badge.icon(icon);
    } else {
        let count = node.props.get("count").and_then(Value::as_u64).unwrap_or(0) as usize;
        badge = badge.count(count);
        if let Some(max) = node.props.get("max").and_then(Value::as_u64) {
            badge = badge.max(max as usize);
        }
    }
    badge
}

/// Props: `label`, `variant` (primary, secondary, danger, success, warning, info), `outline`,
/// `icon`.
pub fn tag(node: &Node) -> Tag {
    let mut tag = match prop_str(node, "variant") {
        Some("primary") => Tag::primary(),
        Some("danger") => Tag::danger(),
        Some("success") => Tag::success(),
        Some("warning") => Tag::warning(),
        Some("info") => Tag::info(),
        _ => Tag::secondary(),
    };
    if prop_bool(node, "outline") {
        tag = tag.outline();
    }
    if let Some(icon) = icon(node, "icon") {
        tag = tag.child(icon);
    }
    let label = prop_str(node, "label").unwrap_or("");
    tag.child(SharedString::from(label.to_owned()))
}

/// Props: `keys` (a key such as `ctrl-s`, or several separated by `+`), `outline`.
pub fn kbd(node: &Node) -> Option<Kbd> {
    let keys = prop_str(node, "keys")?;
    let stroke = crate::keys::parse(keys)?;
    let kbd = Kbd::new(stroke);
    Some(if prop_bool(node, "outline") {
        kbd.outline()
    } else {
        kbd
    })
}

/// Props: `value` (0 to 100), `loading`.
pub fn progress(id: NodeId, node: &Node) -> Progress {
    let value = prop_f32(node, "value").unwrap_or(0.).clamp(0., 100.);
    Progress::new(("node", id))
        .value(value)
        .loading(prop_bool(node, "loading"))
}

/// A spinner. It takes its size from `class` (`size-4`).
pub fn spinner() -> Spinner {
    Spinner::new()
}

/// Props: `secondary`. It takes its size from `class` (`h-4 w-32`).
pub fn skeleton(node: &Node) -> Skeleton {
    let skeleton = Skeleton::new();
    if prop_bool(node, "secondary") {
        skeleton.secondary()
    } else {
        skeleton
    }
}

/// Props: `value` (stars), `max` (default 5), `disabled`, `controlled`, `a11y`. Events: `change`
/// with the chosen value.
pub fn rating(id: NodeId, node: &Node, session: &Entity<Session>) -> Rating {
    let max = node
        .props
        .get("max")
        .and_then(Value::as_u64)
        .unwrap_or(5)
        .max(1) as usize;
    let value = node
        .props
        .get("value")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .min(max as u64) as usize;
    let session = session.clone();
    Rating::new(("node", id))
        .max(max)
        .value(value)
        .disabled(prop_bool(node, "disabled"))
        .on_click(move |chosen, _, cx| {
            let chosen = *chosen;
            if chosen != value {
                session.update(cx, |session, cx| {
                    if value_chosen(&mut session.tree, id, Value::from(chosen as u64)) {
                        cx.notify();
                    }
                });
            }
        })
}

/// Props: `page` (1-based), `total` (pages, default 1), `compact`, `disabled`, `controlled`.
/// Events: `change` with the chosen page.
pub fn pagination(id: NodeId, node: &Node, session: &Entity<Session>) -> Pagination {
    let total = node
        .props
        .get("total")
        .and_then(Value::as_u64)
        .unwrap_or(1)
        .max(1) as usize;
    let page = node
        .props
        .get("page")
        .and_then(Value::as_u64)
        .unwrap_or(1)
        .clamp(1, total as u64) as usize;
    let pagination = Pagination::new(("node", id))
        .total_pages(total)
        .current_page(page);
    let pagination = if prop_bool(node, "compact") {
        pagination.compact()
    } else {
        pagination
    };
    let session = session.clone();
    pagination.on_click(move |chosen, _, cx| {
        let chosen = *chosen;
        if chosen == page {
            return;
        }
        session.update(cx, |session, cx| {
            if prop_chosen(&mut session.tree, id, "page", Value::from(chosen as u64)) {
                cx.notify();
            }
        });
    })
}

/// Props: `open`. Its children are the content; the trigger is the client's (a `Button` that
/// sets `open`).
pub fn collapsible(id: NodeId, node: &Node) -> Collapsible {
    Collapsible::new()
        .open(prop_bool(node, "open"))
        .motion_id(("collapsible", id))
}

/// Props: `name` (shown as its initial when there is no `src`), `src` (an image file).
pub fn avatar(node: &Node) -> Avatar {
    let mut avatar = Avatar::new();
    if let Some(name) = prop_str(node, "name") {
        avatar = avatar.name(name.to_owned());
    }
    if let Some(src) = prop_str(node, "src").filter(|src| !src.is_empty()) {
        avatar = avatar.src(ImageSource::from(std::path::PathBuf::from(src)));
    }
    avatar
}

/// Props: `items` (the step labels, as `{value, label, disabled?}`), `value` (the current step),
/// `vertical`, `disabled`, `controlled`. Events: `change` with the chosen step's `value`.
pub fn stepper(id: NodeId, node: &Node, session: &Entity<Session>) -> Stepper {
    let steps = options(node, "items");
    let current = prop_str(node, "value").map(str::to_owned);
    let selected = current
        .as_deref()
        .and_then(|value| steps.iter().position(|(step, ..)| step == value));
    let values: Vec<String> = steps.iter().map(|(value, ..)| value.clone()).collect();
    let session = session.clone();
    let mut stepper = Stepper::new(("node", id))
        .items(steps.into_iter().map(|(_, label, disabled)| {
            StepperItem::new()
                .disabled(disabled)
                .child(SharedString::from(label))
        }))
        .disabled(prop_bool(node, "disabled"));
    if prop_bool(node, "vertical") {
        stepper = stepper.vertical();
    }
    if let Some(ix) = selected {
        stepper = stepper.selected_index(ix);
    }
    stepper.on_click(move |ix, _, cx| {
        let Some(value) = values
            .get(*ix)
            .filter(|value| Some(*value) != current.as_ref())
        else {
            return;
        };
        session.update(cx, |session, cx| {
            if value_chosen(&mut session.tree, id, Value::String(value.clone())) {
                cx.notify();
            }
        });
    })
}

// Simple presentational components (PROTOCOL.md section 23): label, empty, shimmer.

/// Props: `content`, `secondary`, `masked`, `highlight`. With `highlight`, every occurrence of
/// the substring is highlighted, case-insensitively.
pub fn label(node: &Node) -> Label {
    let label = Label::new(prop_str(node, "content").unwrap_or("").to_owned())
        .masked(prop_bool(node, "masked"));
    let label = match prop_str(node, "secondary") {
        Some(secondary) => label.secondary(secondary.to_owned()),
        None => label,
    };
    match prop_str(node, "highlight") {
        Some(highlight) => label.highlights(highlight.to_owned()),
        None => label,
    }
}

/// Props: `icon`, `title`, `description`; `content` is the children (an action, usually), which
/// go in the component's content slot.
pub fn empty(node: &Node, content: Vec<AnyElement>) -> Empty {
    let mut header = EmptyHeader::new();
    if let Some(icon) = icon(node, "icon") {
        header = header.media(
            EmptyMedia::new()
                .with_variant(EmptyMediaVariant::Icon)
                .child(icon),
        );
    }
    if let Some(title) = prop_str(node, "title") {
        header = header.title(EmptyTitle::new().child(SharedString::from(title.to_owned())));
    }
    if let Some(description) = prop_str(node, "description") {
        header = header
            .description(EmptyDescription::new().child(SharedString::from(description.to_owned())));
    }
    Empty::new()
        .header(header)
        .content(EmptyContent::new().children(content))
}

/// Props: `content`, `duration` (seconds, default 2), `spread` (default 0.3), `reverse`, `once`.
/// It animates on its own, without an `animate-*` class; the crate clamps `spread` to 0.05–1 and
/// a negative `duration` to 1 ms.
pub fn shimmer(id: NodeId, node: &Node) -> ShimmerText {
    let mut shimmer = ShimmerText::new(prop_str(node, "content").unwrap_or("").to_owned())
        .id(("node", id))
        .reverse(prop_bool(node, "reverse"))
        .once(prop_bool(node, "once"));
    if let Some(duration) = prop_f32(node, "duration") {
        shimmer = shimmer.duration(Duration::from_secs_f32(duration.max(0.)));
    }
    if let Some(spread) = prop_f32(node, "spread") {
        shimmer = shimmer.spread(spread);
    }
    shimmer
}
