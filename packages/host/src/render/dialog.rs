//! Dialog nodes, shown with gpui-component's `open_dialog` so they get its overlay, Escape,
//! focus trap and layout; the body is drawn by the window's `RootView`.

use gpui_kit::component::WindowExt;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::dialog::{DialogAction, DialogClose, DialogFooter};
use gpui_kit::*;
use serde_json::Value;

use super::RootView;
use crate::Session;
use crate::components;
use crate::protocol::NodeId;

/// The body view: re-renders when the session changes.
struct Body {
    view: WeakEntity<RootView>,
    id: NodeId,
    _observe: Subscription,
}

impl Render for Body {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (id, owner) = (self.id, cx.entity().entity_id());
        self.view
            .update(cx, |view, cx| {
                view.render_dialog_body(id, owner, window, cx)
            })
            .unwrap_or_else(|_| div().into_any_element())
    }
}

pub fn open(
    id: NodeId,
    session: &Entity<Session>,
    window: &mut Window,
    cx: &mut Context<RootView>,
) {
    let view = cx.entity().downgrade();
    let body = cx.new(|cx| Body {
        view: view.clone(),
        id,
        _observe: cx.observe(session, |_, _, cx| cx.notify()),
    });
    let session = session.clone();
    // Runs again whenever the dialog layer rebuilds, so prop changes show while it is open.
    window.open_dialog(cx, move |dialog, _, cx| {
        let tree = &session.read(cx).tree;
        let text = |key| {
            tree.node(id)
                .and_then(|node| node.props.get(key))
                .and_then(Value::as_str)
                .map(SharedString::from)
        };
        let width = tree
            .node(id)
            .and_then(|node| node.props.get("width"))
            .and_then(Value::as_f64);
        let mut footer = DialogFooter::new();
        if let Some(cancel) = text("cancelText") {
            footer = footer.child(DialogClose::new().child(Button::new("cancel").label(cancel)));
        }
        let ok = text("okText").unwrap_or_else(|| "OK".into());
        footer = footer.child(DialogAction::new().child(Button::new("ok").primary().label(ok)));
        let mut dialog = dialog.child(body.clone()).footer(footer);
        if let Some(title) = text("title") {
            dialog = dialog.title(title);
        }
        if let Some(width) = width {
            dialog = dialog.w(px(width as f32));
        }
        let (ok_session, ok_view) = (session.clone(), view.clone());
        let (cancel_session, cancel_view) = (session.clone(), view.clone());
        dialog
            .on_ok(move |_, _, cx| answered(&ok_session, &ok_view, id, "ok", cx))
            .on_cancel(move |_, _, cx| answered(&cancel_session, &cancel_view, id, "cancel", cx))
    });
}

/// OK, or Cancel (also Escape, the X and a click outside): reports it unless the client
/// closed the dialog itself, and lets it close.
fn answered(
    session: &Entity<Session>,
    view: &WeakEntity<RootView>,
    id: NodeId,
    event: &str,
    cx: &mut App,
) -> bool {
    let ours = view
        .update(cx, |view, _| {
            (view.open_dialog == Some(id)).then(|| view.open_dialog = None)
        })
        .ok()
        .flatten();
    if ours.is_some() {
        session.update(cx, |session, cx| {
            components::dialog_answered(&mut session.tree, id, event);
            cx.notify();
        });
    }
    true
}
