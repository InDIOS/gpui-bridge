//! One root view per window: walks the retained tree from that window's root and builds GPUI elements.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Once;
use std::time::{Duration, Instant};

use gpui_kit::component::form::Field;
use gpui_kit::component::menu::ContextMenuExt;
use gpui_kit::component::scroll::{ScrollableElement, Scrollbar};
use gpui_kit::component::theme::{ActiveTheme, Theme};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{Icon, VirtualListScrollHandle, WindowExt, v_virtual_list};
use gpui_kit::*;
use serde_json::Value;

use crate::Session;
use crate::a11y;
use crate::animation::{self, Timing, Transition, Visual};
use crate::components::calendar::{self, CalendarSlot};
use crate::components::carousel::{self, CarouselSlot};
use crate::components::chart::{self, ChartSlot};
use crate::components::command::{self, CommandSlot};
use crate::components::diff::{self, DiffSlot};
use crate::components::input::{self, InputSlot};
use crate::components::otp::{self, OtpSlot};
use crate::components::select::{self, SelectSlot};
use crate::components::slider::{self, SliderSlot};
use crate::components::table::{self, TableSlot};
use crate::components::{
    self, accordion, basic, button, canvas, checkbox, keyboard, layout, menu, radio, settings,
    sidebar, switch, tabs, tree,
};
use crate::keys;
use crate::protocol::{HostMessage, NodeId, WindowId as ClientWindowId};
use crate::schema::NodeType;
use crate::styles::{self, Class, ScrollAxis, StyleCache};
use crate::transport;
use crate::tree::{Node, Tree};
use gpui_kit::component::input::{InputMode, TextareaMode};

pub struct RootView {
    session: Entity<Session>,
    window_id: ClientWindowId,
    styles: StyleCache,
    inputs: HashMap<NodeId, InputSlot<InputMode>>,
    textareas: HashMap<NodeId, InputSlot<TextareaMode>>,
    /// Number inputs share the text machinery; only the element differs.
    numbers: HashMap<NodeId, InputSlot<InputMode>>,
    selects: HashMap<NodeId, SelectSlot>,
    sliders: HashMap<NodeId, SliderSlot>,
    tables: HashMap<NodeId, TableSlot>,
    /// One per OtpInput and per Calendar: their state keeps what the user typed or chose.
    otps: HashMap<NodeId, OtpSlot>,
    calendars: HashMap<NodeId, CalendarSlot>,
    trees: HashMap<NodeId, tree::TreeSlot>,
    diffs: HashMap<NodeId, DiffSlot>,
    carousels: HashMap<NodeId, CarouselSlot>,
    /// One per Chart: its data, parsed once per change (a series can be 1 000 points long).
    charts: HashMap<NodeId, ChartSlot>,
    /// One per Command: the query and highlight the user typed and moved.
    commands: HashMap<NodeId, CommandSlot>,
    /// One per VirtualList: the list keeps its scroll position across frames.
    lists: HashMap<NodeId, VirtualListScrollHandle>,
    /// Commit whose `value_writes` this window already applied.
    applied_commit: u64,
    /// Nodes with `transition`, by the state of their last frame. Every pass of the window (the
    /// tree, a Dialog body, the visible rows of a VirtualList) shares it.
    transitions: HashMap<NodeId, Transition>,
    /// Hovered nodes among those whose `hover:` the host animates (GPUI's own `hover` cannot be).
    hovered: Rc<RefCell<HashSet<NodeId>>>,
    /// One view per node that keeps moving on its own (an `animate-*` loop, see
    /// `animated_nodes`), kept up to date by `sync_animated`. A frame redraws those nodes and not
    /// the whole window: a spinner on a tree of thousands of nodes should not rebuild all of it.
    animated: HashMap<NodeId, Entity<AnimatedNode>>,
    /// Text content per node, kept between frames (see `RenderCx::text`).
    texts: HashMap<NodeId, SharedString>,
    /// Paces this window's animation frames, shared with the views of its nodes (see `FrameClock`).
    clock: Rc<RefCell<FrameClock>>,
    /// Nodes the last frame produced state for (a transition, or a tracked `hover:`, in any pass).
    /// The next frame keeps only those, so a node removed from the tree, a row scrolled out of a
    /// list or a Dialog that closed forgets its state.
    stateful: HashSet<NodeId>,
    /// Time origin of `animate-*` loops.
    epoch: Instant,
    /// Renders since `frame_second`, to report frames/s with `--debug` (see `render`).
    frames: u32,
    frame_second: Instant,
    /// Time spent building the window's own pass since `frame_second`, and the nodes it built
    /// (both reported with `RUST_LOG=debug`, see `render`).
    pass: Duration,
    built: u32,
    /// The view's own focus: keys reach its shortcut listener even when no control has the focus.
    focus: FocusHandle,
    /// Divs with `keyDown`, which can take the focus.
    focusables: HashMap<NodeId, FocusHandle>,
    /// The Dialog node shown in gpui-component's dialog layer, if any.
    open_dialog: Option<NodeId>,
    /// Last window state reported to the client, so `focus`/`blur` and friends are only sent on
    /// changes. `None` until the first observer call, which primes it (see `track_window`).
    window_active: Option<bool>,
    window_visible: Option<bool>,
    /// Last bounds reported, as `x, y, width, height`.
    window_bounds: Option<(f32, f32, f32, f32)>,
    _observe_session: Subscription,
}

impl RootView {
    pub(crate) fn new(
        session: Entity<Session>,
        window_id: ClientWindowId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        // The session notifies after each commit (and on host-side changes); sync and re-render then.
        let _observe_session = cx.observe_in(&session, window, |this, _, window, cx| {
            this.sync_inputs(window, cx);
            cx.notify();
        });
        let mut this = Self {
            session,
            window_id,
            styles: StyleCache::default(),
            inputs: HashMap::new(),
            textareas: HashMap::new(),
            numbers: HashMap::new(),
            selects: HashMap::new(),
            sliders: HashMap::new(),
            tables: HashMap::new(),
            otps: HashMap::new(),
            calendars: HashMap::new(),
            trees: HashMap::new(),
            diffs: HashMap::new(),
            carousels: HashMap::new(),
            charts: HashMap::new(),
            commands: HashMap::new(),
            lists: HashMap::new(),
            applied_commit: 0,
            transitions: HashMap::new(),
            hovered: Rc::default(),
            animated: HashMap::new(),
            texts: HashMap::new(),
            clock: Rc::default(),
            stateful: HashSet::new(),
            epoch: Instant::now(),
            frames: 0,
            frame_second: Instant::now(),
            pass: Duration::ZERO,
            built: 0,
            focus: cx.focus_handle(),
            focusables: HashMap::new(),
            open_dialog: None,
            window_active: None,
            window_visible: None,
            window_bounds: None,
            _observe_session,
        };
        // Root focus first, so a `focus` request in the commit that opens the window overrides it.
        this.focus.focus(window, cx);
        this.sync_inputs(window, cx);
        this.track_window(window, cx);
        this
    }

    /// Reports `focus`/`blur`, `resize`/`move` and `visibility` to the client when it asked for
    /// them in `window.events` (PROTOCOL.md section 3). The observers call back once when
    /// registered, with the current state: that call primes the stored state, and only a real
    /// change is sent (an opening window does report `focus`, so the app knows it is active).
    /// The subscriptions are detached: they live as long as this view, which lives as long as
    /// its window.
    fn track_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // The visibility observer does not call back when it is registered, so its state is
        // primed here; the bounds one is primed too, so opening a window reports no geometry.
        // `window_bounds` is the frame's origin (what `x`/`y` set) with the client size, which
        // is what the client can feed back into `window`; `bounds()` is the client area, so its
        // origin carries the title bar (it would make a saved position drift).
        self.window_visible = Some(window.visibility().is_visible());
        self.window_bounds = Some(bounds_of(window));

        cx.observe_window_activation(window, |this, window, cx| {
            let active = window.is_window_active();
            let Some(previous) = this.window_active.replace(active) else {
                // First call: the state is primed. An opening window reports `focus`, so the
                // app knows it is active without asking.
                if active {
                    this.send_window_event("focus", None, None, cx);
                }
                return;
            };
            if previous != active {
                this.send_window_event(if active { "focus" } else { "blur" }, None, None, cx);
            }
        })
        .detach();

        cx.observe_window_bounds(window, |this, window, cx| {
            let state = bounds_of(window);
            let Some(previous) = this.window_bounds.replace(state) else {
                return;
            };
            if previous == state {
                return;
            }
            // A drag-resize changes both; report the size, then the position on the next change.
            let event = if (previous.2, previous.3) != (state.2, state.3) {
                "resize"
            } else {
                "move"
            };
            this.send_window_event(event, Some(state), None, cx);
        })
        .detach();

        cx.observe_window_visibility(window, |this, visibility, _window, cx| {
            let visible = visibility.is_visible();
            if this.window_visible.replace(visible) != Some(visible) {
                this.send_window_event("visibility", None, Some(visible), cx);
            }
        })
        .detach();
    }

    /// Sends one window event, if this window subscribed to it. `bounds` is
    /// `x, y, width, height`, for `move` and `resize`.
    fn send_window_event(
        &self,
        event: &str,
        bounds: Option<(f32, f32, f32, f32)>,
        visible: Option<bool>,
        cx: &mut Context<Self>,
    ) {
        if !self
            .session
            .read(cx)
            .tree
            .wants_window_event(self.window_id, event)
        {
            return;
        }
        let (x, y, width, height) = match bounds {
            Some((x, y, width, height)) => (Some(x), Some(y), Some(width), Some(height)),
            None => (None, None, None, None),
        };
        transport::send(&HostMessage::WindowEvent {
            id: self.window_id,
            event: event.to_owned(),
            x,
            y,
            width,
            height,
            visible,
        });
    }

    /// Keeps one state per Input, Textarea, Select and Slider in this window. Text values
    /// imposed by the client apply once per commit, since the session also notifies on
    /// host-side changes (a checkbox click) between commits.
    fn sync_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let tree = &self.session.read(cx).tree;
        let members = self
            .root(tree)
            .map(|root| tree.subtree(root))
            .unwrap_or_default();
        // Only a commit can change what loops in `class`, so the extra walk is skipped for the
        // host's own notifications (a keystroke in an Input, a checkbox click).
        let loops = (tree.commits() != self.applied_commit).then(|| {
            self.root(tree)
                .map(|root| animated_nodes(tree, root))
                .unwrap_or_default()
        });
        let writes = if tree.commits() == self.applied_commit {
            &[][..]
        } else {
            tree.value_writes()
        };
        let inputs = input::snapshot(tree, NodeType::Input, &members, writes);
        let textareas = input::snapshot(tree, NodeType::Textarea, &members, writes);
        let numbers = input::snapshot(tree, NodeType::NumberInput, &members, writes);
        let otps = otp::snapshot(tree, &members);
        let calendars = calendar::snapshot(tree, &members);
        let selects = select::snapshot(tree, &members);
        let sliders = slider::snapshot(tree, &members);
        let tables = table::snapshot(tree, &members);
        let trees = tree::snapshot(tree, &members);
        let diffs = diff::snapshot(tree, &members);
        let carousels = carousel::snapshot(tree, &members);
        let charts = chart::snapshot(tree, &members);
        let commands = command::snapshot(tree, &members);
        let lists: HashSet<NodeId> = members
            .iter()
            .copied()
            .filter(|&id| {
                tree.node(id)
                    .is_some_and(|node| node.node_type == NodeType::VirtualList)
            })
            .collect();
        let focusable: HashSet<NodeId> = members
            .iter()
            .copied()
            .filter(|&id| {
                tree.node(id).is_some_and(|node| {
                    (node.node_type == NodeType::Div && node.events.contains("keyDown"))
                        || keyboard::takes_stop(node)
                })
            })
            .collect();
        self.applied_commit = tree.commits();
        self.focusables.retain(|id, _| focusable.contains(id));
        for id in focusable {
            self.focusables
                .entry(id)
                .or_insert_with(|| cx.focus_handle().tab_stop(true));
        }
        self.lists.retain(|id, _| lists.contains(id));
        for id in lists {
            self.lists
                .entry(id)
                .or_insert_with(VirtualListScrollHandle::new);
        }
        self.texts.retain(|id, _| tree.node(*id).is_some());
        if let Some(loops) = loops {
            self.sync_animated(loops, cx);
        }
        input::sync(&mut self.inputs, inputs, &self.session, window, cx);
        input::sync(&mut self.textareas, textareas, &self.session, window, cx);
        input::sync(&mut self.numbers, numbers, &self.session, window, cx);
        otp::sync(&mut self.otps, otps, &self.session, window, cx);
        calendar::sync(&mut self.calendars, calendars, &self.session, window, cx);
        select::sync(&mut self.selects, selects, &self.session, window, cx);
        slider::sync(&mut self.sliders, sliders, &self.session, window, cx);
        table::sync(&mut self.tables, tables, &self.session, window, cx);
        tree::sync(&mut self.trees, trees, &self.session, cx);
        diff::sync(&mut self.diffs, diffs, &self.session, cx);
        carousel::sync(&mut self.carousels, carousels, &self.session, cx);
        chart::sync(&mut self.charts, charts, &self.session, cx);
        command::sync(&mut self.commands, commands, window, cx);
        // Deferred: the dialog layer and the focus need the whole window, which does not exist
        // yet while `RootView::new` runs (gpui-component's Root wraps the view afterwards).
        cx.defer_in(window, |this, window, cx| {
            this.sync_dialog_and_focus(window, cx)
        });
    }

    /// Keeps one view per node that redraws on its own (see `animated_nodes`): created when a
    /// commit brings the node in, dropped when it leaves the window.
    fn sync_animated(&mut self, ids: HashSet<NodeId>, cx: &mut Context<Self>) {
        self.animated.retain(|id, _| ids.contains(id));
        if self.animated.len() == ids.len() {
            return;
        }
        let view = cx.entity().downgrade();
        let session = self.session.clone();
        let clock = self.clock.clone();
        for id in ids {
            self.animated.entry(id).or_insert_with(|| {
                cx.new(|cx| AnimatedNode::new(view.clone(), id, session.clone(), clock.clone(), cx))
            });
        }
    }

    /// Opens or closes the Dialog the tree asks for, then applies a pending `focus` (after the
    /// dialog, so a field inside it can take the focus).
    fn sync_dialog_and_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let tree = &self.session.read(cx).tree;
        let members = self
            .root(tree)
            .map(|root| tree.subtree(root))
            .unwrap_or_default();
        let focus_request = tree.focus_request().filter(|id| members.contains(id));
        // One Dialog at a time: the open one with the lowest id.
        let wanted_dialog = members
            .iter()
            .copied()
            .filter(|&id| {
                tree.node(id).is_some_and(|node| {
                    node.node_type == NodeType::Dialog
                        && node.props.get("open") == Some(&Value::Bool(true))
                })
            })
            .min();
        if self.open_dialog != wanted_dialog {
            // Forgotten first, so closing it from here is not reported as the user's answer.
            if self.open_dialog.take().is_some() {
                window.close_dialog(cx);
            }
            if let Some(id) = wanted_dialog {
                self.open_dialog = Some(id);
                dialog::open(id, &self.session, window, cx);
            }
        }
        if let Some(id) = focus_request {
            if let Some(slot) = self.inputs.get(&id) {
                slot.focus(window, cx);
            } else if let Some(slot) = self.textareas.get(&id) {
                slot.focus(window, cx);
            } else if let Some(slot) = self.numbers.get(&id) {
                slot.focus(window, cx);
            } else if let Some(slot) = self.otps.get(&id) {
                slot.focus(window, cx);
            } else if let Some(slot) = self.calendars.get(&id) {
                slot.focus(window, cx);
            } else if let Some(slot) = self.trees.get(&id) {
                slot.focus(window, cx);
            } else if let Some(handle) = self.focusables.get(&id) {
                handle.focus(window, cx);
            } else if let Some(slot) = self.tables.get(&id) {
                slot.focus(window, cx);
            } else if let Some(slot) = self.commands.get(&id) {
                slot.focus(window, cx);
            }
            self.session
                .update(cx, |session, _| session.tree.clear_focus_request());
        }
    }

    fn root(&self, tree: &Tree) -> Option<NodeId> {
        tree.windows()
            .get(&self.window_id)
            .and_then(|spec| spec.root)
    }
}

impl RootView {
    /// A render pass over this window's nodes. It works on the view's transitions and hovered set,
    /// so every pass (the tree, a Dialog body, the visible rows of a VirtualList) shares them.
    /// `owner` is the view the pass belongs to: `RootView`, or the `AnimatedNode` of one node.
    fn render_cx<'a>(&'a mut self, cx: &'a Context<Self>, owner: EntityId) -> RenderCx<'a> {
        let now = Instant::now();
        let hovered = self.hovered.clone();
        RenderCx {
            tree: &self.session.read(cx).tree,
            session: &self.session,
            view: cx.entity(),
            owner,
            styles: &mut self.styles,
            inputs: &self.inputs,
            textareas: &self.textareas,
            selects: &self.selects,
            sliders: &self.sliders,
            tables: &self.tables,
            numbers: &self.numbers,
            otps: &self.otps,
            calendars: &self.calendars,
            trees: &self.trees,
            diffs: &self.diffs,
            carousels: &self.carousels,
            charts: &self.charts,
            commands: &self.commands,
            lists: &self.lists,
            texts: &mut self.texts,
            animated: &self.animated,
            theme: cx.theme(),
            in_control: false,
            in_chip: false,
            transitions: &mut self.transitions,
            hovered,
            stateful: &mut self.stateful,
            focusables: &self.focusables,
            now,
            elapsed: now.saturating_duration_since(self.epoch),
            animating: false,
            built: 0,
        }
    }

    /// The rows a VirtualList shows now.
    fn render_rows(
        &mut self,
        rows: &[NodeId],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        // The view this pass belongs to: the list's, which draws its rows.
        let owner = window.current_view();
        let mut ctx = self.render_cx(cx, owner);
        // A one-cell grid stretches each row to the list's width and `rowHeight`, as a list item is.
        let rows = rows
            .iter()
            .map(|&row| {
                div()
                    .grid()
                    .size_full()
                    .child(ctx.node(row))
                    .into_any_element()
            })
            .collect();
        let animating = ctx.animating;
        drop(ctx);
        if animating {
            self.clock.borrow_mut().next_frame(window, cx, owner);
        }
        rows
    }

    /// Builds one node that redraws on its own for its view (`AnimatedNode`), without handing it
    /// back to a view of its own. `owner` is that view. The flag says whether it still moves, so
    /// the view knows to ask for the next frame.
    fn render_animated(
        &mut self,
        id: NodeId,
        owner: EntityId,
        cx: &Context<Self>,
    ) -> (AnyElement, bool) {
        let mut ctx = self.render_cx(cx, owner);
        let element = ctx.node_raw(id);
        (element, ctx.animating)
    }

    /// A Dialog's children, for its body in the dialog layer, with the Dialog's classes on the
    /// container. Built like `render`, as a separate pass. `owner` is the body's own view, which is
    /// the one that has to re-render when a `hover:` inside it changes.
    fn render_dialog_body(
        &mut self,
        id: NodeId,
        owner: EntityId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut ctx = self.render_cx(cx, owner);
        let Some(node) = ctx.tree.node(id) else {
            return div().into_any_element();
        };
        let classes = ctx.styles.classes(
            node.props
                .get("class")
                .and_then(Value::as_str)
                .unwrap_or(""),
        );
        let children: Vec<AnyElement> =
            node.children.iter().map(|&child| ctx.node(child)).collect();
        let body = ctx
            .style(id, div().flex().flex_col().gap_2(), &classes, false)
            .children(children);
        let animating = ctx.animating;
        drop(ctx);
        if animating {
            self.clock.borrow_mut().next_frame(window, cx, owner);
        }
        body.into_any_element()
    }
}

mod dialog;

/// One animation frame every 60th of a second, at most: a spinner or a fade does not need more, and
/// GPUI asks for the next frame as fast as the loop allows when a frame is cheap.
const FRAME_INTERVAL: Duration = Duration::from_micros(16_667);

/// Paces a window's animation frames. GPUI draws again as soon as a view asks
/// (`request_animation_frame`, which notifies the view that is rendering), so a window whose frames
/// are cheap runs far above the display rate: measured at 180 frames/s, and ~45 % of a core, for a
/// window with nothing but one spinner.
///
/// Here every view that asks is notified once per `FRAME_INTERVAL`, together, from one timer: one
/// frame per interval for the whole window, however many nodes move. A frame that takes longer than
/// the interval is followed by the next one at once, so a heavy tree is not slowed down further.
#[derive(Default)]
struct FrameClock {
    /// When the views that asked are due to see the next frame.
    due: Option<Instant>,
    /// Views that asked for a frame since the last one; all of them are notified when it is due.
    pending: Rc<RefCell<HashSet<EntityId>>>,
    /// A timer is already going for what is in `pending`.
    scheduled: Rc<Cell<bool>>,
}

impl FrameClock {
    /// Asks for the next frame of `view`.
    fn next_frame(&mut self, window: &mut Window, cx: &App, view: EntityId) {
        self.pending.borrow_mut().insert(view);
        if self.scheduled.replace(true) {
            return;
        }
        let now = Instant::now();
        let due = self.due.unwrap_or(now).max(now);
        self.due = Some(due + FRAME_INTERVAL);
        let (pending, scheduled) = (self.pending.clone(), self.scheduled.clone());
        window
            .spawn(cx, async move |cx| {
                cx.background_executor()
                    .timer(due.saturating_duration_since(Instant::now()))
                    .await;
                scheduled.set(false);
                let views: Vec<EntityId> = pending.borrow_mut().drain().collect();
                cx.update(move |_, cx| {
                    for view in views {
                        cx.notify(view);
                    }
                })
                .ok();
            })
            .detach();
    }
}

/// A node whose `animate-*` loop runs until the class goes away, drawn by a view of its own. A frame
/// it asks for notifies this view (the one rendering), so the next frame rebuilds this node instead
/// of the whole window.
///
/// Note that GPUI also marks the *ancestors* of a notified view as dirty, so the window's own view
/// still runs its pass on every frame; what this saves is the work below the node, not the walk
/// itself (measured in HISTORIAL.md, "Rendimiento: redibujado incremental").
struct AnimatedNode {
    view: WeakEntity<RootView>,
    id: NodeId,
    /// Props and classes come from the session (a commit, or a host-side change): re-render then.
    _observe: Subscription,
    /// The window's frame pacing: every node of a window shares it, or each spinner would ask for
    /// its own frame.
    clock: Rc<RefCell<FrameClock>>,
    /// Frames this node drew since `frame_second`, for the `RUST_LOG=debug` report.
    frames: u32,
    frame_second: Instant,
}

impl AnimatedNode {
    fn new(
        view: WeakEntity<RootView>,
        id: NodeId,
        session: Entity<Session>,
        clock: Rc<RefCell<FrameClock>>,
        cx: &mut Context<Self>,
    ) -> Self {
        let _observe = cx.observe(&session, |_, _, cx| cx.notify());
        AnimatedNode {
            view,
            id,
            _observe,
            clock,
            frames: 0,
            frame_second: Instant::now(),
        }
    }
}

impl Render for AnimatedNode {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.frames += 1;
        let since = self.frame_second.elapsed();
        if since >= Duration::from_secs(1) {
            log::debug!(
                "node {} {:.1} frames/s",
                self.id,
                self.frames as f32 / since.as_secs_f32()
            );
            self.frames = 0;
            self.frame_second = Instant::now();
        }
        let (id, owner) = (self.id, cx.entity().entity_id());
        let (element, animating) = self
            .view
            .update(cx, |view, cx| view.render_animated(id, owner, cx))
            .unwrap_or_else(|_| (div().into_any_element(), false));
        if animating {
            // This view is the innermost one rendering, so the frame goes to it alone.
            self.clock.borrow_mut().next_frame(window, cx, owner);
        }
        element
    }
}

impl Render for RootView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // How many frames the window really draws and what its own pass costs, with
        // `RUST_LOG=debug`: the way to tell an animation loop from a tree that is rebuilt far more
        // often than it needs to be.
        self.frames += 1;
        let since = self.frame_second.elapsed();
        if since >= Duration::from_secs(1) {
            let frames = self.frames as f32;
            log::debug!(
                "window {}: {:.1} frames/s, {:.2} ms and {} nodes per pass",
                self.window_id,
                frames / since.as_secs_f32(),
                self.pass.as_secs_f32() * 1000. / frames,
                self.built / self.frames.max(1)
            );
            self.frames = 0;
            self.pass = Duration::ZERO;
            self.built = 0;
            self.frame_second = Instant::now();
        }
        // Clicking where nothing takes the focus leaves none; take it back so shortcuts keep working.
        if window.focused(cx).is_none() {
            self.focus.focus(window, cx);
        }
        // A frame starts here. What no pass produced state for in the last frame forgets its
        // transition and its `hover:` (a node removed from the tree, a row scrolled out of a list,
        // a Dialog that closed).
        let keep = std::mem::take(&mut self.stateful);
        self.transitions.retain(|id, _| keep.contains(id));
        self.hovered.borrow_mut().retain(|id| keep.contains(id));
        let (session, window_id, focus) =
            (self.session.clone(), self.window_id, self.focus.clone());
        // The nodes that move on their own are drawn by their own views (see `AnimatedNode`), so
        // what this pass asks for a frame for is only what is left of the tree.
        let mut ctx = self.render_cx(cx, cx.entity().entity_id());
        let root = ctx
            .tree
            .windows()
            .get(&window_id)
            .and_then(|spec| spec.root);
        // Capture phase: a shortcut wins over the focused control (registering ctrl-c stops Input copying).
        let shortcuts = move |event: &KeyDownEvent, _: &mut Window, cx: &mut App| {
            if keys::pressed(&session.read(cx).tree, window_id, &event.keystroke) {
                cx.stop_propagation();
            }
        };
        let started = Instant::now();
        let children: Option<AnyElement> = root.map(|root| ctx.node(root));
        let (pass, built, animating) = (started.elapsed(), ctx.built, ctx.animating);
        drop(ctx);
        self.pass += pass;
        self.built += built;
        let element = div()
            .size_full()
            .track_focus(&focus)
            .capture_key_down(shortcuts)
            .children(children);
        // The window's own focus (when nothing else has it) needs an accessibility node too.
        let element = {
            let tree = &self.session.read(cx).tree;
            let title = tree
                .windows()
                .get(&window_id)
                .and_then(|spec| spec.title.clone())
                .unwrap_or_default();
            element
                .id(("window", window_id))
                .role(Role::Window)
                .aria_label(title)
        };
        if animating {
            // Something in this pass moves, so the window draws another frame; the rest of the
            // window is only redrawn when a commit, the pointer or the theme changes it.
            self.clock
                .borrow_mut()
                .next_frame(window, cx, cx.entity().entity_id());
        }
        element
    }
}

struct RenderCx<'a> {
    tree: &'a Tree,
    session: &'a Entity<Session>,
    /// The window's view, which draws a VirtualList's visible rows.
    view: Entity<RootView>,
    /// The view this pass belongs to, which a hover on a tracked node re-renders: `RootView`, or
    /// the `AnimatedNode` of one node.
    owner: EntityId,
    styles: &'a mut StyleCache,
    inputs: &'a HashMap<NodeId, InputSlot<InputMode>>,
    textareas: &'a HashMap<NodeId, InputSlot<TextareaMode>>,
    selects: &'a HashMap<NodeId, SelectSlot>,
    sliders: &'a HashMap<NodeId, SliderSlot>,
    tables: &'a HashMap<NodeId, TableSlot>,
    numbers: &'a HashMap<NodeId, InputSlot<InputMode>>,
    otps: &'a HashMap<NodeId, OtpSlot>,
    calendars: &'a HashMap<NodeId, CalendarSlot>,
    trees: &'a HashMap<NodeId, tree::TreeSlot>,
    diffs: &'a HashMap<NodeId, DiffSlot>,
    carousels: &'a HashMap<NodeId, CarouselSlot>,
    charts: &'a HashMap<NodeId, ChartSlot>,
    commands: &'a HashMap<NodeId, CommandSlot>,
    lists: &'a HashMap<NodeId, VirtualListScrollHandle>,
    /// Text content per node, kept between frames (see `RenderCx::text`).
    texts: &'a mut HashMap<NodeId, SharedString>,
    /// Nodes that draw themselves, each in a view of its own (see `animated_nodes`).
    animated: &'a HashMap<NodeId, Entity<AnimatedNode>>,
    theme: &'a Theme,
    /// Inside a `windowControl` area, where interactive nodes must block it (see `blocks`).
    in_control: bool,
    /// Inside a `Tab` chip, where the control of a node keeps its own click so it does not
    /// choose the tab (see `blocks_area`; the chip's own text, icons and `Badge`s are part of
    /// the chip and still choose it, so `component_a11y` does not block them).
    in_chip: bool,
    /// Every node's transition, by the state of its last frame. Shared with the view, so the pass
    /// a Dialog body or a list row runs in keeps it too.
    transitions: &'a mut HashMap<NodeId, Transition>,
    hovered: Rc<RefCell<HashSet<NodeId>>>,
    /// Nodes this pass produced state for (a transition, or a tracked `hover:`).
    stateful: &'a mut HashSet<NodeId>,
    focusables: &'a HashMap<NodeId, FocusHandle>,
    now: Instant,
    /// Since the window opened, for `animate-*`.
    elapsed: Duration,
    /// Something moves: the window must draw another frame.
    animating: bool,
    /// Nodes this pass built, for the `RUST_LOG=debug` report of `RootView::render`.
    built: u32,
}

static WARN_SPIN: Once = Once::new();

impl RenderCx<'_> {
    fn node(&mut self, id: NodeId) -> AnyElement {
        // A node that moves on its own is drawn by its view, which redraws it alone on each frame.
        match self.animated.get(&id) {
            Some(view) => view.clone().into_any_element(),
            None => self.node_raw(id),
        }
    }

    /// Builds the node here, even if it has a view of its own (that is what its view does).
    ///
    /// Each node type is built by a function of its own. In an unoptimized build the frame of a
    /// single function holding every type was about 200 KB, reserved on every level of this walk,
    /// so a tree nested five levels deep overflowed the 1 MB stack of the main thread
    /// (STATUS_STACK_OVERFLOW, exit code 3221225725) before drawing its first frame.
    fn node_raw(&mut self, id: NodeId) -> AnyElement {
        self.built += 1;
        let Some(node) = self.tree.node(id) else {
            // The window root may point at a node that does not exist (yet).
            return div().into_any_element();
        };
        let class = node
            .props
            .get("class")
            .and_then(Value::as_str)
            .unwrap_or("");
        let classes = self.styles.classes(class);
        if node.node_type != NodeType::Icon && classes.contains(&Class::AnimateSpin) {
            WARN_SPIN.call_once(|| {
                log::warn!("'animate-spin' only rotates Icon nodes; ignored elsewhere")
            });
        }

        let accessibility = a11y::of(node);
        if keyboard::takes_stop(node)
            && let Some(handle) = self.focusables.get(&id).cloned()
        {
            return self.keyboard_stop(id, node, &classes, &handle, accessibility);
        }
        let element = match node.node_type {
            NodeType::Div => self.div_node(id, node, &classes),
            NodeType::MenuBar => self.menu_bar_node(id, node, &classes),
            NodeType::Image => self.image_node(id, node, &classes),
            NodeType::Icon => self.icon_node(id, node, &classes),
            NodeType::Text => self.text_node(id, node, &classes),
            NodeType::Button => self.button_node(id, node, &classes),
            NodeType::Checkbox => self.checkbox_node(id, node, &classes),
            NodeType::Switch => self.switch_node(id, node, &classes),
            NodeType::Tabs => self.tabs_node(id, node, &classes, accessibility.as_ref()),
            // Drawn by its `Tabs`: on its own a `Tab` is not a chip (PROTOCOL.md section 15).
            NodeType::Tab => div().into_any_element(),
            NodeType::Table => self.table_node(id, node, &classes),
            NodeType::VirtualList => self.virtual_list_node(id, node, &classes),
            // Drawn in gpui-component's dialog layer (`dialog::open`), not where it sits in the tree.
            NodeType::Dialog => div().into_any_element(),
            NodeType::RadioGroup => self.radio_group_node(id, node, &classes),
            NodeType::Select => self.select_node(id, node, &classes),
            NodeType::Slider => self.slider_node(id, node, &classes),
            NodeType::Textarea => self.textarea_node(id, node, &classes),
            NodeType::Input => self.input_node(id, node, &classes),
            NodeType::NumberInput => self.number_input_node(id, node, &classes),
            NodeType::OtpInput => self.otp_input_node(id, node, &classes),
            NodeType::Alert => self.component(id, basic::alert(id, node, self.session), &classes),
            NodeType::Badge => self.badge_node(id, node, &classes),
            NodeType::Tag => self.component(id, basic::tag(node), &classes),
            NodeType::Kbd => self.kbd_node(id, node, &classes),
            NodeType::Progress => self.component(id, basic::progress(id, node), &classes),
            NodeType::Spinner => self.component(id, basic::spinner(), &classes),
            NodeType::Skeleton => self.component(id, basic::skeleton(node), &classes),
            NodeType::Rating => self.component(id, basic::rating(id, node, self.session), &classes),
            NodeType::Pagination => {
                self.component(id, basic::pagination(id, node, self.session), &classes)
            }
            NodeType::Collapsible => self.collapsible_node(id, node, &classes),
            NodeType::Accordion => {
                self.component(id, accordion::build(id, node, self.session), &classes)
            }
            NodeType::Avatar => self.component(id, basic::avatar(node), &classes),
            NodeType::Stepper => {
                self.component(id, basic::stepper(id, node, self.session), &classes)
            }
            NodeType::Form => self.form_node(id, node, &classes),
            NodeType::Field => self.field_node(id, node, &classes),
            NodeType::GroupBox => self.group_box_node(id, node, &classes),
            NodeType::DescriptionList => {
                self.component(id, layout::description_list(node), &classes)
            }
            NodeType::StatusBar => self.component(id, layout::status_bar(node), &classes),
            NodeType::Toolbar => self.toolbar_node(id, node, &classes),
            NodeType::Resizable => self.resizable_node(id, node, &classes),
            NodeType::Calendar => self.calendar_node(id, node, &classes),
            NodeType::Tree => self.tree_node(id, node, &classes, accessibility.as_ref()),
            NodeType::Diff => self.diff_node(id, node, &classes, accessibility.as_ref()),
            NodeType::Carousel => self.carousel_node(id, node, &classes, accessibility.as_ref()),
            NodeType::Sidebar => self.sidebar_node(id, node, &classes, accessibility.as_ref()),
            NodeType::Chart => self.chart_node(id, node, &classes, accessibility.as_ref()),
            NodeType::Canvas => self.canvas_node(id, node, &classes, accessibility.as_ref()),
            NodeType::Command => self.command_node(id, node, &classes),
            NodeType::Settings => {
                let element = settings::build(id, node, self.session);
                self.component_a11y(id, element, &classes, accessibility.as_ref())
            }
        };
        // `div` and `text` apply `a11y` themselves; `Tabs` and the data components put it on their
        // own sized wrapper (an extra flex wrapper would shrink a `size_full` component to nothing).
        // Any other component cannot take an id or a role from outside, so it gets a wrapper
        // (only when the client asked for one).
        match accessibility {
            Some(spec)
                if !matches!(
                    node.node_type,
                    NodeType::Div
                        | NodeType::Text
                        | NodeType::Tabs
                        | NodeType::Tree
                        | NodeType::Diff
                        | NodeType::Carousel
                        | NodeType::Sidebar
                        | NodeType::Chart
                        | NodeType::Canvas
                ) =>
            {
                a11y::wrap(&spec, id, element)
            }
            _ => element,
        }
    }

    /// The children of `node`, in order.
    fn children(&mut self, node: &Node) -> Vec<AnyElement> {
        node.children
            .iter()
            .map(|&child| self.node(child))
            .collect()
    }

    /// Whether an interactive node built here keeps its own click and blocks whatever holds it:
    /// inside a `windowControl` area, or inside a `Tab` chip, where it must not choose the tab.
    fn blocks_area(&self) -> bool {
        self.in_control || self.in_chip
    }

    /// A component that does not take the node's classes itself: a wrapper holds them, and it
    /// blocks a `windowControl` area.
    fn component(
        &mut self,
        id: NodeId,
        element: impl IntoElement + 'static,
        classes: &[Class],
    ) -> AnyElement {
        self.component_a11y(id, element, classes, None)
    }

    /// A component `keyboard` gives a Tab stop: the stop is its sized wrapper (as in
    /// `component_a11y`) and carries the accessibility node, so the reader follows the focus.
    fn keyboard_stop(
        &mut self,
        id: NodeId,
        node: &Node,
        classes: &[Class],
        handle: &FocusHandle,
        client: Option<a11y::A11y>,
    ) -> AnyElement {
        let element = match node.node_type {
            NodeType::Slider => self.slider_node(id, node, &[]),
            NodeType::Tabs => self.tabs_node(id, node, &[], None),
            NodeType::Rating => self.component(id, basic::rating(id, node, self.session), &[]),
            _ => self.component(id, basic::pagination(id, node, self.session), &[]),
        };
        let spec = keyboard::spec(self.tree, node, client);
        let wrapper = if self.in_control {
            div().occlude()
        } else {
            div()
        };
        // Inset: an outer shadow fills the transparent wrapper instead of drawing a ring.
        let ring = BoxShadow {
            color: self.theme.ring,
            offset: point(px(0.), px(0.)),
            blur_radius: px(0.),
            spread_radius: px(2.),
            inset: true,
        };
        let session = self.session.clone();
        let stop = a11y::apply(
            &spec,
            id,
            wrapper.rounded(self.theme.radius).track_focus(handle),
        )
        .focus(move |style| style.shadow(vec![ring]))
        .on_key_down(move |event: &KeyDownEvent, _, cx| {
            let taken = session.update(cx, |session, cx| {
                let taken = keyboard::key_down(&mut session.tree, id, &event.keystroke);
                if taken {
                    cx.notify();
                }
                taken
            });
            if taken {
                cx.stop_propagation();
            }
        });
        self.style(id, stop, classes, false)
            .child(element)
            .into_any_element()
    }

    /// `component` for a node whose `a11y` goes on the same sized wrapper. The general
    /// `a11y::wrap` is a flex row, which gives a `size_full` component no width to size against.
    fn component_a11y(
        &mut self,
        id: NodeId,
        element: impl IntoElement + 'static,
        classes: &[Class],
        a11y: Option<&a11y::A11y>,
    ) -> AnyElement {
        let wrapper = if self.in_control {
            div().occlude()
        } else {
            div()
        };
        match a11y {
            Some(spec) => self
                .style(id, a11y::apply(spec, id, wrapper), classes, false)
                .child(element)
                .into_any_element(),
            None => self
                .style(id, wrapper, classes, false)
                .child(element)
                .into_any_element(),
        }
    }

    fn badge_node(&mut self, id: NodeId, node: &Node, classes: &[Class]) -> AnyElement {
        let children = self.children(node);
        self.component(id, basic::badge(node).children(children), classes)
    }

    fn kbd_node(&mut self, id: NodeId, node: &Node, classes: &[Class]) -> AnyElement {
        match basic::kbd(node) {
            Some(kbd) => self.component(id, kbd, classes),
            None => self.style(id, div(), classes, false).into_any_element(),
        }
    }

    // `Collapsible` holds one element: the children go in a column of their own.
    fn collapsible_node(&mut self, id: NodeId, node: &Node, classes: &[Class]) -> AnyElement {
        let children = self.children(node);
        let content = div().flex().flex_col().children(children);
        self.component(id, basic::collapsible(id, node).content(content), classes)
    }

    fn number_input_node(&mut self, id: NodeId, node: &Node, classes: &[Class]) -> AnyElement {
        match self.numbers.get(&id) {
            Some(slot) => self.blocking(id, input::build_number(node, slot), classes),
            None => div().into_any_element(),
        }
    }

    fn otp_input_node(&mut self, id: NodeId, node: &Node, classes: &[Class]) -> AnyElement {
        match self.otps.get(&id) {
            Some(slot) => self.component(id, otp::build(node, slot), classes),
            None => div().into_any_element(),
        }
    }

    fn field_node(&mut self, id: NodeId, node: &Node, classes: &[Class]) -> AnyElement {
        let children = self.children(node);
        self.component(id, layout::field(node, children), classes)
    }

    // A `Form` takes `Field` children; any other child goes in one without a label.
    fn form_node(&mut self, id: NodeId, node: &Node, classes: &[Class]) -> AnyElement {
        let mut fields = Vec::with_capacity(node.children.len());
        for &child in &node.children {
            match self
                .tree
                .node(child)
                .filter(|child| child.node_type == NodeType::Field)
            {
                Some(field) => {
                    let children = self.children(field);
                    fields.push(layout::field(field, children));
                }
                None => fields.push(Field::new().child(self.node(child))),
            }
        }
        self.component(id, layout::form(node, fields), classes)
    }

    fn group_box_node(&mut self, id: NodeId, node: &Node, classes: &[Class]) -> AnyElement {
        let children = self.children(node);
        self.component(id, layout::group_box(id, node).children(children), classes)
    }

    fn toolbar_node(&mut self, id: NodeId, node: &Node, classes: &[Class]) -> AnyElement {
        let children = self.children(node);
        self.component(id, layout::toolbar(id).contents(children), classes)
    }

    fn resizable_node(&mut self, id: NodeId, node: &Node, classes: &[Class]) -> AnyElement {
        let panels = self.children(node);
        self.component(id, layout::resizable(id, node, panels), classes)
    }

    fn calendar_node(&mut self, id: NodeId, node: &Node, classes: &[Class]) -> AnyElement {
        match self.calendars.get(&id) {
            Some(slot) => self.component(id, calendar::build(node, slot), classes),
            None => div().into_any_element(),
        }
    }

    fn tree_node(
        &mut self,
        id: NodeId,
        _node: &Node,
        classes: &[Class],
        a11y: Option<&a11y::A11y>,
    ) -> AnyElement {
        match self.trees.get(&id) {
            Some(slot) => {
                self.component_a11y(id, tree::build(id, slot, self.session), classes, a11y)
            }
            None => div().into_any_element(),
        }
    }

    /// The component keeps its own scrolling: it needs a bounded height in `class`.
    fn diff_node(
        &mut self,
        id: NodeId,
        node: &Node,
        classes: &[Class],
        a11y: Option<&a11y::A11y>,
    ) -> AnyElement {
        match self.diffs.get(&id) {
            Some(slot) => self.component_a11y(id, diff::build(node, slot), classes, a11y),
            None => div().into_any_element(),
        }
    }

    /// One child per slide, in order.
    fn carousel_node(
        &mut self,
        id: NodeId,
        node: &Node,
        classes: &[Class],
        a11y: Option<&a11y::A11y>,
    ) -> AnyElement {
        let slides = self.children(node);
        match self.carousels.get(&id) {
            Some(slot) => {
                self.component_a11y(id, carousel::build(id, node, slot, slides), classes, a11y)
            }
            None => div().into_any_element(),
        }
    }

    /// Its content is the `items` prop: the node takes no children. It needs a bounded height
    /// (`h-full` inside a row, or `class`), and the wrapper is that box.
    fn sidebar_node(
        &mut self,
        id: NodeId,
        node: &Node,
        classes: &[Class],
        a11y: Option<&a11y::A11y>,
    ) -> AnyElement {
        let element = sidebar::build(id, node, self.session);
        self.component_a11y(id, element, classes, a11y)
    }

    /// The plot fills the box (`h-64` in `class`), which the wrapper provides.
    fn chart_node(
        &mut self,
        id: NodeId,
        node: &Node,
        classes: &[Class],
        a11y: Option<&a11y::A11y>,
    ) -> AnyElement {
        match self.charts.get(&id) {
            Some(slot) => {
                let element = chart::build(id, node, slot, self.theme);
                self.component_a11y(id, element, classes, a11y)
            }
            None => div().into_any_element(),
        }
    }

    fn canvas_node(
        &mut self,
        id: NodeId,
        node: &Node,
        classes: &[Class],
        a11y: Option<&a11y::A11y>,
    ) -> AnyElement {
        let element = canvas::build(id, node, self.theme, self.session);
        self.component_a11y(id, element, classes, a11y)
    }

    fn div_node(&mut self, id: NodeId, node: &Node, classes: &[Class]) -> AnyElement {
        let control = window_control(node);
        let blocks = self.blocks_area() && (control.is_some() || node.events.contains("click"));
        let outer = self.in_control;
        self.in_control |= control.is_some();
        let children: Vec<AnyElement> = node
            .children
            .iter()
            .map(|&child| self.node(child))
            .collect();
        self.in_control = outer;
        // With `transition`, the host follows the pointer itself to animate `hover:`.
        let track_hover = Timing::of(classes).is_some() && styles::has_hover(classes);
        let hovered = track_hover && self.hovered.borrow().contains(&id);
        let mut element = self.style(id, div(), classes, hovered).children(children);
        if !track_hover {
            element = styles::apply_hover(element, classes, self.theme);
        }
        if let Some(area) = control {
            element = element.window_control_area(area);
        }
        if blocks {
            element = element.occlude();
        }
        if let Some(handle) = self.focusables.get(&id) {
            element = self.key_down(id, element.track_focus(handle));
            // Focus on click, unless the click focused something inside (an Input).
            let handle = handle.clone();
            element = element.on_mouse_down(MouseButton::Left, move |_, window, cx| {
                if !handle.contains_focused(window, cx) {
                    handle.focus(window, cx);
                }
            });
        }
        if node.events.contains("drop") {
            let session = self.session.clone();
            element = element.on_drop(move |dropped: &ExternalPaths, _, cx| {
                let paths = dropped
                    .paths()
                    .iter()
                    .map(|path| path.to_string_lossy().into_owned())
                    .collect();
                components::dropped(&session.read(cx).tree, id, paths);
            });
        }
        let axis = styles::scroll_axis(classes);
        let menu = node
            .props
            .get("contextMenu")
            .and_then(Value::as_array)
            .filter(|items| !items.is_empty());
        let menu = menu.map(|items| menu::builder(items.clone(), self.session, id));
        let clicks = node.events.contains("click") || node.events.contains("doubleClick");
        let tooltip = node
            .props
            .get("tooltip")
            .and_then(Value::as_str)
            .map(SharedString::from);
        let a11y = a11y::of(node);
        // An accessibility node needs an id too, so the div gets one even with nothing to click.
        if clicks || menu.is_some() || track_hover || tooltip.is_some() || a11y.is_some() {
            // A stable id keeps the context menu's open state across renders.
            let mut element = element.id(("node", id));
            if clicks {
                let session = self.session.clone();
                element = element.on_click(move |event, _, cx| {
                    components::emit_click(&session, id, cx);
                    if event.click_count() == 2 {
                        components::double_clicked(&session.read(cx).tree, id);
                    }
                });
            }
            if let Some(tooltip) = tooltip {
                element = element
                    .tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx));
            }
            if track_hover {
                self.stateful.insert(id);
                let (hovered, owner) = (self.hovered.clone(), self.owner);
                element = element.on_hover(move |&inside, _, cx| {
                    let changed = if inside {
                        hovered.borrow_mut().insert(id)
                    } else {
                        hovered.borrow_mut().remove(&id)
                    };
                    if changed {
                        // Only the view that drew the node re-renders: `window.refresh`
                        // would drop every cached subtree in the window, a list's rows
                        // included, for a hover that changes one color.
                        cx.notify(owner);
                    }
                });
            }
            if let Some(spec) = &a11y {
                element = a11y::apply_stateful(spec, element);
            }
            scrollable(element, axis, id, menu)
        } else {
            scrollable(element, axis, id, None)
        }
    }

    fn menu_bar_node(&mut self, id: NodeId, node: &Node, classes: &[Class]) -> AnyElement {
        let menus = node
            .props
            .get("menus")
            .and_then(Value::as_array)
            .map_or(&[][..], Vec::as_slice);
        let bar = menu::bar(id, menus, self.session);
        let bar = if self.blocks_area() {
            bar.occlude()
        } else {
            bar
        };
        self.style(id, bar, classes, false).into_any_element()
    }

    fn image_node(&mut self, id: NodeId, node: &Node, classes: &[Class]) -> AnyElement {
        // Bytes sent as `data` win over `src`; the host keeps them in a file per node.
        let has_data = node
            .props
            .get("data")
            .and_then(Value::as_str)
            .is_some_and(|data| !data.is_empty());
        let path = if has_data {
            self.tree.image_path(id)
        } else {
            node.props
                .get("src")
                .and_then(Value::as_str)
                .filter(|src| !src.is_empty())
                .map(PathBuf::from)
        };
        let Some(path) = path else {
            return self.style(id, div(), classes, false).into_any_element();
        };
        let fit = match node.props.get("fit").and_then(Value::as_str) {
            Some("cover") => ObjectFit::Cover,
            Some("fill") => ObjectFit::Fill,
            Some("scale-down") => ObjectFit::ScaleDown,
            Some("none") => ObjectFit::None,
            _ => ObjectFit::Contain,
        };
        // A PathBuf, not a string: GPUI would read `C:\...` as a URL with scheme `c`.
        self.style(id, img(path).object_fit(fit), classes, false)
            .into_any_element()
    }

    fn icon_node(&mut self, id: NodeId, node: &Node, classes: &[Class]) -> AnyElement {
        let name = node.props.get("name").and_then(Value::as_str).unwrap_or("");
        let mut icon = Icon::default().path(components::icon_path(name));
        if classes.contains(&Class::AnimateSpin) {
            icon = icon.rotate(animation::spin(self.elapsed));
            self.animating = true;
        }
        self.style(id, icon, classes, false).into_any_element()
    }

    fn text_node(&mut self, id: NodeId, node: &Node, classes: &[Class]) -> AnyElement {
        let content = self.text(id, node);
        let element = self.style(id, div(), classes, false).child(content);
        match a11y::of(node) {
            Some(spec) => a11y::apply(&spec, id, element).into_any_element(),
            None => element.into_any_element(),
        }
    }

    // Inside a `windowControl` area, a control must block it or Windows takes its clicks as title bar ones.
    fn button_node(&mut self, id: NodeId, node: &Node, classes: &[Class]) -> AnyElement {
        let button = button::build(id, node, self.session);
        let button = if self.blocks_area() {
            button.occlude()
        } else {
            button
        };
        self.style(id, button, classes, false).into_any_element()
    }

    fn checkbox_node(&mut self, id: NodeId, node: &Node, classes: &[Class]) -> AnyElement {
        let checkbox = checkbox::build(id, node, self.session);
        let checkbox = if self.blocks_area() {
            checkbox.occlude()
        } else {
            checkbox
        };
        self.style(id, checkbox, classes, false).into_any_element()
    }

    fn switch_node(&mut self, id: NodeId, node: &Node, classes: &[Class]) -> AnyElement {
        let switch = switch::build(id, node, self.session);
        self.blocking(id, switch, classes)
    }

    /// A `Tabs` with `Tab` children takes its tabs from the tree and ignores `tabs`. Its `a11y`
    /// goes on a sized wrapper, so the bar still fills the width its parent gives it.
    fn tabs_node(
        &mut self,
        id: NodeId,
        node: &Node,
        classes: &[Class],
        a11y: Option<&a11y::A11y>,
    ) -> AnyElement {
        let mut chips = Vec::new();
        for &child in &node.children {
            let Some(tab) = self
                .tree
                .node(child)
                .filter(|tab| tab.node_type == NodeType::Tab)
            else {
                continue;
            };
            let mut content = self.tab_content(tab);
            let suffix = tab
                .children
                .last()
                .and_then(|&last| self.tree.node(last))
                .filter(|last| tabs::is_control(last))
                .and_then(|_| content.pop());
            let (value, disabled, chip) = tabs::chip(tab, content, suffix);
            // The chip is the `Tab` node, so its `class` styles the chip itself (`flex-1` makes
            // the tabs share the bar's width).
            let class = tab.props.get("class").and_then(Value::as_str).unwrap_or("");
            let classes = self.styles.classes(class);
            chips.push((value, disabled, styles::apply(chip, &classes, self.theme)));
        }
        let bar = tabs::build(id, node, chips, self.session);
        match a11y {
            Some(spec) => self.component_a11y(id, bar, classes, Some(&tabs::named(spec.clone()))),
            None => self.blocking(id, bar, classes),
        }
    }

    /// The content of a `Tab`: here the control of a node keeps its own click, so clicking a
    /// `Button` in a tab does not choose the tab (see `in_chip`).
    fn tab_content(&mut self, node: &Node) -> Vec<AnyElement> {
        let outer = std::mem::replace(&mut self.in_chip, true);
        let children = self.children(node);
        self.in_chip = outer;
        children
    }

    // A wrapper takes the classes (DataTable fills it) and blocks a `windowControl` area.
    fn table_node(&mut self, id: NodeId, node: &Node, classes: &[Class]) -> AnyElement {
        match self.tables.get(&id) {
            Some(slot) => {
                let wrapper = if self.blocks_area() {
                    div().occlude()
                } else {
                    div()
                };
                self.style(id, wrapper, classes, false)
                    .child(table::build(node, slot))
                    .into_any_element()
            }
            None => div().into_any_element(),
        }
    }

    // Only the rows in view are built (`render_rows`); the list fills its container, as gpui-component's List does.
    fn virtual_list_node(&mut self, id: NodeId, node: &Node, classes: &[Class]) -> AnyElement {
        match self.lists.get(&id) {
            Some(scroll) => {
                let height = px(node
                    .props
                    .get("rowHeight")
                    .and_then(Value::as_f64)
                    .unwrap_or(32.) as f32);
                let rows = node.children.clone();
                let sizes = Rc::new(vec![size(px(0.), height); rows.len()]);
                let list = v_virtual_list(
                    self.view.clone(),
                    ("vlist", id),
                    sizes,
                    move |view, range, window, cx| {
                        view.render_rows(rows.get(range).unwrap_or_default(), window, cx)
                    },
                )
                .track_scroll(scroll);
                self.style(id, div().relative().overflow_hidden(), classes, false)
                    .child(list)
                    .child(Scrollbar::vertical(scroll))
                    .into_any_element()
            }
            None => div().into_any_element(),
        }
    }

    fn radio_group_node(&mut self, id: NodeId, node: &Node, classes: &[Class]) -> AnyElement {
        // A wrapper takes the classes: RadioGroup lays out its own radios.
        let group = radio::build(id, node, self.session);
        let wrapper = if self.blocks_area() {
            div().occlude()
        } else {
            div()
        };
        self.style(id, wrapper, classes, false)
            .child(group)
            .into_any_element()
    }

    // Stateful controls without `occlude`: a blocking wrapper takes their classes inside a `windowControl` area.
    fn select_node(&mut self, id: NodeId, node: &Node, classes: &[Class]) -> AnyElement {
        match self.selects.get(&id) {
            Some(slot) => self.blocking(id, select::build(node, slot), classes),
            None => div().into_any_element(),
        }
    }

    fn command_node(&mut self, id: NodeId, node: &Node, classes: &[Class]) -> AnyElement {
        match self.commands.get(&id) {
            Some(slot) => {
                let element = command::build(id, node, slot, self.session);
                self.blocking(id, element, classes)
            }
            None => div().into_any_element(),
        }
    }

    fn slider_node(&mut self, id: NodeId, node: &Node, classes: &[Class]) -> AnyElement {
        match self.sliders.get(&id) {
            Some(slot) => self.blocking(id, slider::build(node, slot), classes),
            None => div().into_any_element(),
        }
    }

    fn textarea_node(&mut self, id: NodeId, node: &Node, classes: &[Class]) -> AnyElement {
        match self.textareas.get(&id) {
            Some(slot) => self.blocking(id, input::build_textarea(node, slot), classes),
            None => div().into_any_element(),
        }
    }

    fn input_node(&mut self, id: NodeId, node: &Node, classes: &[Class]) -> AnyElement {
        match self.inputs.get(&id) {
            // Input cannot occlude itself or listen to keys: a wrapper takes its classes instead.
            Some(slot) if self.blocks_area() || node.events.contains("keyDown") => {
                let wrapper = if self.blocks_area() {
                    div().occlude().flex()
                } else {
                    div().flex()
                };
                let wrapper = self.key_down(id, wrapper);
                self.style(id, wrapper, classes, false)
                    .child(input::build(node, slot))
                    .into_any_element()
            }
            Some(slot) => self
                .style(id, input::build(node, slot), classes, false)
                .into_any_element(),
            None => div().into_any_element(),
        }
    }

    /// A Text node's content, kept as a `SharedString` from the frame before: building a `String`
    /// per node per frame cost about a third of the pass for a tree of 20 000 texts (measured).
    fn text(&mut self, id: NodeId, node: &Node) -> SharedString {
        let raw = node
            .props
            .get("content")
            .and_then(Value::as_str)
            .unwrap_or("");
        match self.texts.get(&id) {
            Some(text) if text.as_ref() == raw => text.clone(),
            _ => {
                let text = SharedString::from(raw.to_owned());
                self.texts.insert(id, text.clone());
                text
            }
        }
    }

    /// Reports `keyDown` while the element or something inside it has the focus. Capture phase,
    /// so the focused control still gets the key afterwards.
    fn key_down(&self, id: NodeId, element: Div) -> Div {
        let session = self.session.clone();
        element.capture_key_down(move |event: &KeyDownEvent, _, cx| {
            components::key_down(&session.read(cx).tree, id, &event.keystroke);
        })
    }

    /// A control that cannot `occlude` itself: where it must block what holds it a blocking
    /// wrapper takes its classes; elsewhere it takes them itself.
    fn blocking<E: Styled + IntoElement + 'static>(
        &mut self,
        id: NodeId,
        element: E,
        classes: &[Class],
    ) -> AnyElement {
        if self.blocks_area() {
            self.style(id, div().occlude().flex(), classes, false)
                .child(element)
                .into_any_element()
        } else {
            self.style(id, element, classes, false).into_any_element()
        }
    }

    /// `styles::apply`, then the colors and opacity of a running `transition` and `animate-pulse` on top.
    fn style<E: Styled>(&mut self, id: NodeId, element: E, classes: &[Class], hovered: bool) -> E {
        let element = styles::apply(element, classes, self.theme);
        let mut visual = None;
        if let Some(timing) = Timing::of(classes) {
            let target = Visual::resolve(classes, self.theme, hovered);
            let transition = match self.transitions.remove(&id) {
                Some(mut transition) => {
                    transition.retarget(target, timing, self.now);
                    transition
                }
                None => Transition::at_rest(target, timing, self.now),
            };
            self.animating |= transition.running(self.now);
            visual = Some(transition.current(self.now));
            self.transitions.insert(id, transition);
            self.stateful.insert(id);
        }
        if classes.contains(&Class::AnimatePulse) {
            let visual = visual.get_or_insert_with(|| Visual::resolve(classes, self.theme, false));
            visual.opacity = Some(visual.opacity.unwrap_or(1.) * animation::pulse(self.elapsed));
            self.animating = true;
        }
        match visual {
            Some(visual) => visual.apply(element),
            None => element,
        }
    }
}

/// Nodes that get a render pass of their own, so a frame redraws them and not the whole window.
///
/// They are the `animate-*` loops, which run on every frame until the class goes away, so they are
/// few in any app (a spinner, a skeleton). A `transition` is not one of them: it only moves for the
/// few hundred milliseconds after a commit or a `hover:`, and the pass that draws it asks for those
/// frames (see `RenderCx::style`); a view per transitioning node would be one per list row.
///
/// None inside a `windowControl` area: there what a node renders depends on the area around it (see
/// `blocks` in `RenderCx::node_raw`), and the window's own pass keeps drawing those.
fn animated_nodes(tree: &Tree, root: NodeId) -> HashSet<NodeId> {
    fn walk(tree: &Tree, id: NodeId, in_control: bool, out: &mut HashSet<NodeId>) {
        let Some(node) = tree.node(id) else { return };
        let control = window_control(node).is_some();
        let class = node
            .props
            .get("class")
            .and_then(Value::as_str)
            .unwrap_or("");
        if !in_control && !control && loops(node, class) {
            out.insert(id);
        }
        let in_control = in_control || control;
        for &child in &node.children {
            walk(tree, child, in_control, out);
        }
    }
    let mut out = HashSet::new();
    walk(tree, root, false, &mut out);
    out
}

/// Whether `class` asks for a loop: `animate-pulse` on any node, `animate-spin` (which only turns
/// an `Icon`) included.
fn loops(node: &Node, class: &str) -> bool {
    let spins = node.node_type == NodeType::Icon;
    class
        .split_whitespace()
        .any(|name| name == "animate-pulse" || (spins && name == "animate-spin"))
}

/// The window's frame origin and client size, as `x, y, width, height`: the coordinates a
/// client sets with `window` and can persist from `windowEvent`. `Window::bounds()` is the
/// client area, so its origin differs by the title bar height.
fn bounds_of(window: &Window) -> (f32, f32, f32, f32) {
    let bounds = window.window_bounds().get_bounds();
    (
        f32::from(bounds.origin.x),
        f32::from(bounds.origin.y),
        f32::from(bounds.size.width),
        f32::from(bounds.size.height),
    )
}

/// `windowControl`: Windows treats the area as its own title bar part (drag, snap, buttons).
fn window_control(node: &Node) -> Option<WindowControlArea> {
    match node.props.get("windowControl").and_then(Value::as_str)? {
        "drag" => Some(WindowControlArea::Drag),
        "minimize" => Some(WindowControlArea::Min),
        "maximize" => Some(WindowControlArea::Max),
        "close" => Some(WindowControlArea::Close),
        _ => None,
    }
}

/// Drops the image files the last commit sent from GPUI's cache, so a file changed on disk is read again.
pub fn reload_images(session: &Entity<Session>, cx: &mut App) {
    for path in session.update(cx, |session, _| session.tree.take_image_reloads()) {
        ImageSource::from(path).remove_asset(cx);
    }
}

/// Wraps a `div` in a scroll area with a scrollbar, keyed by node so each keeps its position,
/// and adds its context menu.
fn scrollable<E: ScrollableElement + ParentElement + IntoElement + 'static>(
    element: E,
    axis: Option<ScrollAxis>,
    id: NodeId,
    menu: Option<menu::Builder>,
) -> AnyElement {
    match axis {
        None => with_menu(element, menu),
        Some(ScrollAxis::X) => with_menu(element.overflow_x_scrollbar().id(("scroll", id)), menu),
        Some(ScrollAxis::Y) => with_menu(element.overflow_y_scrollbar().id(("scroll", id)), menu),
        Some(ScrollAxis::Both) => with_menu(element.overflow_scrollbar().id(("scroll", id)), menu),
    }
}

fn with_menu<E: InteractiveElement + ParentElement + Styled + IntoElement + 'static>(
    element: E,
    menu: Option<menu::Builder>,
) -> AnyElement {
    match menu {
        Some(menu) => element
            .context_menu(move |m, window, cx| menu(m, window, cx))
            .into_any_element(),
        None => element.into_any_element(),
    }
}

#[cfg(test)]
mod tests {
    // Explicit imports: `super::*` would bring GPUI's `test` macro, shadowing Rust's.
    use super::animated_nodes;
    use crate::protocol::NodeId;
    use crate::tree::Tree;
    use serde_json::{Value, json};

    /// A tree from `(id, type, props)` creates and `(parent, child)` appends, already committed.
    fn tree(creates: &[(NodeId, &str, Value)], appends: &[(NodeId, NodeId)]) -> Tree {
        let mut tree = Tree::default();
        let queue = |tree: &mut Tree, value: Value| {
            tree.queue(crate::protocol::parse_client_message(&value.to_string()).unwrap());
        };
        for (id, node_type, props) in creates {
            queue(
                &mut tree,
                json!({"op": "create", "id": id, "type": node_type, "props": props}),
            );
        }
        for (parent, child) in appends {
            queue(
                &mut tree,
                json!({"op": "append", "parent": parent, "child": child}),
            );
        }
        assert_eq!(tree.commit(), vec![], "the test tree must apply");
        tree
    }

    #[test]
    fn only_what_moves_on_its_own_gets_a_view() {
        let tree = tree(
            &[
                (1, "div", json!({"class": "flex"})),
                (
                    2,
                    "div",
                    json!({"class": "p-2 transition duration-300 hover:bg-accent"}),
                ),
                (
                    3,
                    "Icon",
                    json!({"name": "loader-circle", "class": "size-4 animate-spin"}),
                ),
                (
                    4,
                    "text",
                    json!({"content": "spinner", "class": "animate-spin"}),
                ),
                (5, "div", json!({"class": "size-4 animate-pulse"})),
                (6, "Icon", json!({"name": "check", "class": "size-4"})),
            ],
            &[(1, 2), (1, 3), (1, 4), (1, 5), (1, 6)],
        );
        let mut animated: Vec<NodeId> = animated_nodes(&tree, 1).into_iter().collect();
        animated.sort_unstable();
        // A spinning Icon and a pulsing div. Not the text (only an Icon can spin), not a still
        // Icon, and not the `transition` div: a transition is drawn by the window's own pass.
        assert_eq!(animated, vec![3, 5]);
    }

    #[test]
    fn a_window_control_area_keeps_the_window_pass() {
        let tree = tree(
            &[
                (1, "div", json!({"class": "flex"})),
                (2, "div", json!({"windowControl": "drag"})),
                (
                    3,
                    "Icon",
                    json!({"name": "loader-circle", "class": "animate-spin"}),
                ),
                (
                    4,
                    "Icon",
                    json!({"name": "loader-circle", "class": "animate-spin"}),
                ),
            ],
            &[(1, 2), (2, 3), (1, 4)],
        );
        // What a node renders inside a title bar area depends on the area, so only node 4, which
        // is outside it, gets a view of its own.
        assert_eq!(
            animated_nodes(&tree, 1).into_iter().collect::<Vec<_>>(),
            vec![4]
        );
    }
}
