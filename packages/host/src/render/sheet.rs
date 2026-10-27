//! The `Sheet` node's window layer (PROTOCOL.md section 24), like the dialog layer next door: a
//! panel over the window with the node's children as its body.

use gpui_kit::component::Placement;
use gpui_kit::component::WindowExt;
use gpui_kit::*;
use serde_json::Value;

use super::{RootView, dialog};
use crate::Session;
use crate::components;
use crate::protocol::NodeId;

/// Opens the panel the node describes. It is a layer of the window, not drawn where the node sits.
pub fn open(
    id: NodeId,
    session: &Entity<Session>,
    window: &mut Window,
    cx: &mut Context<RootView>,
) {
    let body = dialog::body(id, session, cx);
    let placement = side(session, id, cx);
    let view = cx.entity().downgrade();
    let session = session.clone();
    // Runs again whenever the sheet layer rebuilds, so prop changes show while it is open.
    window.open_sheet_at(placement, cx, move |sheet, _window, cx| {
        let (title, size) = title_size(&session, id, cx);
        let mut sheet = sheet.child(body.clone());
        if let Some(title) = title {
            sheet = sheet.title(title);
        }
        if let Some(size) = size {
            sheet = sheet.size(px(size));
        }
        let (closed_session, closed_view) = (session.clone(), view.clone());
        sheet.on_close(move |_, _, cx| closed(&closed_session, &closed_view, id, cx))
    });
}

/// The `side` prop: the edge the panel slides in from, `right` when the node has none.
fn side(session: &Entity<Session>, id: NodeId, cx: &App) -> Placement {
    let tree = &session.read(cx).tree;
    match tree
        .node(id)
        .and_then(|node| node.props.get("side"))
        .and_then(Value::as_str)
    {
        Some("left") => Placement::Left,
        Some("top") => Placement::Top,
        Some("bottom") => Placement::Bottom,
        _ => Placement::Right,
    }
}

/// The `title` and `size` props, read before the window is borrowed mutably: the tree lives behind
/// the session's `read`, which borrows the app.
fn title_size(
    session: &Entity<Session>,
    id: NodeId,
    cx: &App,
) -> (Option<SharedString>, Option<f32>) {
    let tree = &session.read(cx).tree;
    let node = tree.node(id);
    (
        node.and_then(|node| node.props.get("title"))
            .and_then(Value::as_str)
            .map(SharedString::from),
        node.and_then(|node| node.props.get("size"))
            .and_then(Value::as_f64)
            .map(|size| size as f32),
    )
}

/// The panel closed itself (its X, Escape or a click on the overlay), after the crate closed it:
/// reports `close` unless the client closed it, and forgets the node.
fn closed(session: &Entity<Session>, view: &WeakEntity<RootView>, id: NodeId, cx: &mut App) {
    let ours = view
        .update(cx, |view, _| {
            (view.open_sheet == Some(id)).then(|| view.open_sheet = None)
        })
        .ok()
        .flatten();
    if ours.is_some() {
        session.update(cx, |session, cx| {
            components::overlays::sheet_closed(&mut session.tree, id);
            cx.notify();
        });
    }
}
