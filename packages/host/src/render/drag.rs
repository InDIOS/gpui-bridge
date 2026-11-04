//! `drag` on an `absolute` div (PROTOCOL.md section 8): the host moves the node itself, inside its
//! parent and stuck to the edges `magnetic` asks for, and reports where it was left with `dragEnd`.
//! `dragData` on a div: the user carries a copy of it to a `dropTarget` div, which gets `dropNode`.

use std::collections::HashMap;

use gpui_kit::*;
use serde_json::Value;

use super::RootView;
use crate::protocol::NodeId;
use crate::tree::{Node, Tree};

/// GPUI's drag value for a node being moved. Its own type, so a node drop target never takes it.
pub struct NodeDrag(pub NodeId);

/// GPUI's drag value for a node carried to a `dropTarget`, with the client's `dragData`.
pub struct NodeData(pub String);

/// The copy of a carried node that follows the pointer: the node drawn again, see-through.
pub struct Preview {
    pub view: WeakEntity<RootView>,
    pub id: NodeId,
    pub size: Option<Size<Pixels>>,
}

impl Render for Preview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (id, owner) = (self.id, cx.entity().entity_id());
        let copy = self
            .view
            .update(cx, |view, cx| view.render_animated(id, owner, cx).0)
            .unwrap_or_else(|_| div().into_any_element());
        let preview = div().opacity(0.8).child(copy);
        match self.size {
            Some(size) => preview.w(size.width).h(size.height),
            None => preview,
        }
    }
}

/// One window's drags, shared by its passes and the handlers they install.
#[derive(Default)]
pub struct Drags {
    /// Where the user left each node, as `left`/`top` inside the parent's border. It wins over
    /// the classes until the client sets `class` on the node.
    pub placed: HashMap<NodeId, Point<Pixels>>,
    /// The node being dragged, and where the pointer holds it from its top-left corner.
    pub active: Option<(NodeId, Point<Pixels>)>,
    /// The client set `class` on the dragged node: its classes win again when it is dropped.
    pub reset: bool,
    /// The bounds of each parent's children, from the last frame (`on_children_prepainted`).
    pub children: HashMap<NodeId, Vec<Bounds<Pixels>>>,
    /// The size of each `dragData` node in the last frame, which its copy takes.
    pub sizes: HashMap<NodeId, Size<Pixels>>,
    /// The node carried to a `dropTarget`: where the pointer holds it, and its size.
    pub carried: Option<(Point<Pixels>, Option<Size<Pixels>>)>,
    /// The bounds of each `dropTarget` while a node is carried.
    pub targets: HashMap<NodeId, Bounds<Pixels>>,
}

impl Drags {
    /// The client set `class` on these nodes: the classes place them again (on drop, for the one moving).
    pub fn class_set(&mut self, ids: &[NodeId]) {
        for id in ids {
            if self.active.is_some_and(|(active, _)| active == *id) {
                self.reset = true;
            } else {
                self.placed.remove(id);
            }
        }
    }

    /// The drag GPUI ended (on mouse up), if one was going and the node moved: the node and where
    /// it was left. When the client set its classes meanwhile, they place it again.
    pub fn finish(&mut self, still_dragging: bool) -> Option<(NodeId, Point<Pixels>)> {
        if still_dragging {
            return None;
        }
        // Sizes are measured again this frame, and no node is carried any more.
        self.sizes.clear();
        self.carried = None;
        self.targets.clear();
        let (id, _) = self.active.take()?;
        let at = self.placed.get(&id).copied();
        if std::mem::take(&mut self.reset) {
            self.placed.remove(&id);
        }
        Some((id, at?))
    }
}

/// `magnetic` in px: `true` is 8, absent or `false` is 0.
pub fn magnet(node: &Node) -> f32 {
    match node.props.get("magnetic") {
        Some(Value::Bool(true)) => 8.,
        Some(value) => value.as_f64().unwrap_or(0.) as f32,
        None => 0.,
    }
}

pub fn draggable(node: &Node) -> bool {
    node.props.get("drag") == Some(&Value::Bool(true))
}

/// The children of `node` with `drag`, by their place among the children, with their `magnetic`.
pub fn draggable_children(tree: &Tree, node: &Node) -> Vec<(usize, NodeId, f32)> {
    node.children
        .iter()
        .enumerate()
        .filter_map(|(index, &id)| {
            let child = tree.node(id).filter(|child| draggable(child))?;
            Some((index, id, magnet(child)))
        })
        .collect()
}

/// Where a node of `size` dragged to `at` lands inside a parent whose inside is `inner` (all
/// relative to the parent's inside). Within `magnet` px of an edge of the parent, or of a sibling
/// it overlaps (or nearly) on the other axis, it snaps to it: touching the sibling or aligned with
/// it. Then it stays inside the parent. Whole px, so `dragEnd` gives what the client writes back.
pub fn place(
    at: Point<f32>,
    size: Size<f32>,
    inner: Size<f32>,
    siblings: &[Bounds<f32>],
    magnet: f32,
) -> Point<f32> {
    let snap = |pos: f32, len: f32, limit: f32, edges: &mut dyn Iterator<Item = (f32, f32)>| {
        let mut best = (magnet, pos);
        let mut candidates = vec![0., limit - len];
        for (start, end) in edges {
            candidates.extend([start - len, end, start, end - len]);
        }
        for candidate in candidates {
            let distance = (candidate - pos).abs();
            if distance <= best.0 {
                best = (distance, candidate);
            }
        }
        if magnet > 0. { best.1 } else { pos }
    };
    let near = |start: f32, len: f32, other_start: f32, other_end: f32| {
        start < other_end + magnet && start + len > other_start - magnet
    };
    let x = snap(
        at.x,
        size.width,
        inner.width,
        &mut siblings
            .iter()
            .filter(|s| near(at.y, size.height, s.top(), s.bottom()))
            .map(|s| (s.left(), s.right())),
    );
    let y = snap(
        at.y,
        size.height,
        inner.height,
        &mut siblings
            .iter()
            .filter(|s| near(at.x, size.width, s.left(), s.right()))
            .map(|s| (s.top(), s.bottom())),
    );
    point(
        x.min(inner.width - size.width).max(0.).round(),
        y.min(inner.height - size.height).max(0.).round(),
    )
}

#[cfg(test)]
mod tests {
    // Not `super::*`: GPUI's own `test` macro would come with it.
    use super::place;
    use gpui_kit::{Bounds, Size, point, size};

    fn bounds(x: f32, y: f32, w: f32, h: f32) -> Bounds<f32> {
        Bounds::new(point(x, y), size(w, h))
    }

    const INNER: Size<f32> = Size {
        width: 400.,
        height: 300.,
    };
    const TILE: Size<f32> = Size {
        width: 100.,
        height: 60.,
    };

    #[test]
    fn stays_inside_the_parent() {
        let place = |x, y| place(point(x, y), TILE, INNER, &[], 0.);
        assert_eq!(place(-20., -5.), point(0., 0.));
        assert_eq!(place(350., 280.), point(300., 240.));
        assert_eq!(place(10.4, 20.6), point(10., 21.));
        // A node larger than its parent sits at the start.
        assert_eq!(
            super::place(point(5., 5.), size(500., 60.), INNER, &[], 0.),
            point(0., 5.)
        );
    }

    #[test]
    fn snaps_to_the_parent_edges() {
        assert_eq!(place(point(6., 7.), TILE, INNER, &[], 8.), point(0., 0.));
        assert_eq!(
            place(point(295., 9.), TILE, INNER, &[], 8.),
            point(300., 9.)
        );
        assert_eq!(place(point(6., 7.), TILE, INNER, &[], 0.), point(6., 7.));
    }

    #[test]
    fn snaps_to_a_sibling_it_overlaps_on_the_other_axis() {
        let other = [bounds(150., 100., 100., 60.)];
        // Touching its left side, and aligned with its top.
        assert_eq!(
            place(point(55., 104.), TILE, INNER, &other, 8.),
            point(50., 100.)
        );
        // Touching its right side.
        assert_eq!(
            place(point(254., 120.), TILE, INNER, &other, 8.),
            point(250., 120.)
        );
        // Under it, aligned with its left side.
        assert_eq!(
            place(point(146., 163.), TILE, INNER, &other, 8.),
            point(150., 160.)
        );
    }

    #[test]
    fn ignores_a_sibling_far_away_on_the_other_axis() {
        let other = [bounds(150., 200., 100., 60.)];
        // Its left side is 3 px away, but it is 120 px lower.
        assert_eq!(
            place(point(147., 20.), TILE, INNER, &other, 8.),
            point(147., 20.)
        );
    }
}
