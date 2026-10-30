//! `Dock` (PROTOCOL.md section 28, ADR 0003): gpui-component's dock area over the node's
//! `DockPanel` children. The host owns the layout: a panel is added where its `placement` says
//! when it appears and removed when its node goes or is closed; everything in between (drags,
//! splits, sizes) is the user's, so no commit undoes it.

use std::collections::{HashMap, HashSet};

use gpui_kit::component::dock::{
    self, DockArea, DockPlacement, DockSkin, PanelEvent, panel_handle,
};
use gpui_kit::*;

use super::list::RenderItem;
use super::{emit, prop_bool, prop_str};
use crate::Session;
use crate::protocol::NodeId;
use crate::schema::NodeType;
use crate::tree::Tree;

/// The user closed a `DockPanel` (or a `simulate` did): it stays out of its Dock from now on.
pub fn closed(tree: &mut Tree, id: NodeId) {
    tree.close_panel(id);
    emit(tree, id, "close", None, None);
}

/// One `DockPanel` as the dock sees it: its tab reads `title` and `closable` from the tree, and
/// its body is the node's children.
pub struct Panel {
    session: Entity<Session>,
    id: NodeId,
    render: RenderItem,
    focus: FocusHandle,
}

impl EventEmitter<PanelEvent> for Panel {}

impl Focusable for Panel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for Panel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .children((self.render)(self.id, window, cx))
    }
}

impl dock::BasePanel for Panel {
    fn panel_name(&self) -> &'static str {
        "DockPanel"
    }

    fn closable(&self, cx: &App) -> bool {
        let tree = &self.session.read(cx).tree;
        tree.node(self.id)
            .is_some_and(|node| prop_bool(node, "closable"))
    }

    fn on_removed(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        // `sync` removes a panel only once its node is gone or closed, and moving a panel never
        // calls this: anything else is the user closing it.
        let tree = &self.session.read(cx).tree;
        if tree.node(self.id).is_some() && !tree.is_panel_closed(self.id) {
            let id = self.id;
            self.session
                .update(cx, |session, _| closed(&mut session.tree, id));
        }
    }
}

impl dock::Panel for Panel {
    fn title(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tree = &self.session.read(cx).tree;
        SharedString::from(
            tree.node(self.id)
                .and_then(|node| prop_str(node, "title"))
                .unwrap_or_default()
                .to_owned(),
        )
    }
}

pub struct DockSlot {
    area: Entity<DockArea>,
    panels: HashMap<NodeId, Entity<Panel>>,
}

impl DockSlot {
    pub fn area(&self) -> Entity<DockArea> {
        self.area.clone()
    }
}

/// Each Dock among `members`, with its open `DockPanel` children and where each one starts.
pub struct DockSnapshot(Vec<(NodeId, Vec<(NodeId, DockPlacement)>)>);

pub fn snapshot(tree: &Tree, members: &HashSet<NodeId>) -> DockSnapshot {
    DockSnapshot(
        tree.nodes()
            .filter(|(id, node)| node.node_type == NodeType::Dock && members.contains(id))
            .map(|(id, node)| {
                let panels = node
                    .children
                    .iter()
                    .filter_map(|&child| {
                        let panel = tree
                            .node(child)
                            .filter(|panel| panel.node_type == NodeType::DockPanel)?;
                        (!tree.is_panel_closed(child))
                            .then(|| (child, placement(prop_str(panel, "placement"))))
                    })
                    .collect();
                (id, panels)
            })
            .collect(),
    )
}

fn placement(name: Option<&str>) -> DockPlacement {
    match name {
        Some("left") => DockPlacement::Left,
        Some("right") => DockPlacement::Right,
        Some("bottom") => DockPlacement::Bottom,
        _ => DockPlacement::Center,
    }
}

/// Creates dock areas, drops removed ones, adds the new panels and removes the ones whose node
/// went or was closed. Nothing else touches the layout.
pub fn sync<V: 'static>(
    slots: &mut HashMap<NodeId, DockSlot>,
    snapshot: DockSnapshot,
    session: &Entity<Session>,
    render: &RenderItem,
    window: &mut Window,
    cx: &mut Context<V>,
) {
    slots.retain(|id, _| snapshot.0.iter().any(|(dock, _)| dock == id));
    for (id, panels) in snapshot.0 {
        let slot = slots.entry(id).or_insert_with(|| DockSlot {
            area: DockSkin::dock_area(format!("dock-{id}"), None, window, cx).0,
            panels: HashMap::new(),
        });
        let before = slot.panels.len();
        slot.panels.retain(|panel, entity| {
            let keep = panels.iter().any(|(open, _)| open == panel);
            if !keep {
                let entity = entity.clone();
                slot.area
                    .update(cx, |area, cx| area.remove_panel(entity, window, cx));
            }
            keep
        });
        if slot.panels.len() < before {
            // The crate keeps a side dock that lost its last panel as an empty strip.
            slot.area.update(cx, |area, cx| {
                for side in [
                    DockPlacement::Left,
                    DockPlacement::Right,
                    DockPlacement::Bottom,
                ] {
                    if area.has_dock(side) && area.is_empty(side, cx) {
                        area.remove_dock(side, window, cx);
                    }
                }
            });
        }
        for (panel, placement) in panels {
            match slot.panels.get(&panel) {
                // Its content may have changed: draw it again.
                Some(entity) => entity.update(cx, |_, cx| cx.notify()),
                None => {
                    let entity = cx.new(|cx| Panel {
                        session: session.clone(),
                        id: panel,
                        render: render.clone(),
                        focus: cx.focus_handle(),
                    });
                    let view = panel_handle(entity.clone());
                    slot.area.update(cx, |area, cx| {
                        area.add_panel_view(view, placement, None, window, cx)
                    });
                    slot.panels.insert(panel, entity);
                }
            }
        }
    }
}
