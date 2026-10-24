//! Retained node tree. Operations are queued and applied only on `commit`.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt::Write;
use std::path::PathBuf;

use serde_json::Value;

use crate::image_data;
use crate::protocol::{
    ClientMessage, CustomThemes, ErrorCode, NodeId, Props, ProtocolError, WindowId, WindowTheme,
};
use crate::schema::NodeType;

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub node_type: NodeType,
    pub props: Props,
    pub events: BTreeSet<String>,
    pub children: Vec<NodeId>,
    pub parent: Option<NodeId>,
}

/// Window events a client can subscribe to (PROTOCOL.md section 3).
pub const WINDOW_EVENTS: [&str; 5] = ["focus", "blur", "resize", "move", "visibility"];

#[derive(Debug, Clone, Default, PartialEq)]
pub struct WindowSpec {
    pub title: Option<String>,
    pub width: Option<f32>,
    pub height: Option<f32>,
    /// Initial position; only read when the window opens.
    pub x: Option<f32>,
    pub y: Option<f32>,
    /// Minimum size the user can resize to; only read when the window opens.
    pub min_width: Option<f32>,
    pub min_height: Option<f32>,
    pub root: Option<NodeId>,
    /// No system title bar; only read when the window opens.
    pub frameless: Option<bool>,
    /// Events the client asked for; `window.events` replaces the set.
    pub events: BTreeSet<String>,
}

/// The system tray icon (PROTOCOL.md section 11). `revision` grows with every `tray` message,
/// so the host applies it again even when nothing changed (the icon file or a native check mark may have).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TraySpec {
    pub icon: Option<String>,
    pub tooltip: Option<String>,
    pub menu: Option<Value>,
    pub revision: u64,
}

/// The theme is app-wide in gpui-component: any `window` message sets it for every window.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ThemeSpec {
    pub theme: Option<WindowTheme>,
    pub theme_set: Option<CustomThemes>,
}

#[derive(Debug, Default)]
pub struct Tree {
    nodes: HashMap<NodeId, Node>,
    windows: BTreeMap<WindowId, WindowSpec>,
    theme: ThemeSpec,
    tray: Option<TraySpec>,
    /// App-wide shortcuts, as the client wrote them.
    shortcuts: Vec<String>,
    /// A node to focus, until the window that shows it does.
    focus_request: Option<NodeId>,
    /// The last commit removed the tray: with no window open, the host exits.
    tray_removed: bool,
    /// `minimized` requests, applied by `sync_windows` after opening or updating windows:
    /// an action, not a remembered state (the user can restore the window at any time).
    window_requests: Vec<(WindowId, bool)>,
    pending: Vec<ClientMessage>,
    /// Number of commits applied, so each window view applies `value_writes` once.
    commits: u64,
    /// Input nodes whose `value` the client imposed in the last commit (PROTOCOL.md section 7).
    value_writes: Vec<NodeId>,
    /// Image files to read again: a `src` the client sent, or the bytes it sent as `data`.
    image_reloads: Vec<PathBuf>,
    /// Files holding the bytes of `Image` nodes that use `data` (one per node).
    image_data: image_data::Store,
    warned_props: HashSet<(NodeType, String)>,
}

impl Tree {
    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(&id)
    }

    pub fn nodes(&self) -> impl Iterator<Item = (NodeId, &Node)> {
        self.nodes.iter().map(|(&id, node)| (id, node))
    }

    pub fn windows(&self) -> &BTreeMap<WindowId, WindowSpec> {
        &self.windows
    }

    /// Whether the window subscribed to a window event (`window.events`, PROTOCOL.md section 3).
    pub fn wants_window_event(&self, id: WindowId, event: &str) -> bool {
        self.windows
            .get(&id)
            .is_some_and(|spec| spec.events.contains(event))
    }

    /// `minimized` requests of the last commit: `true` minimizes, `false` restores.
    /// `sync_windows` drains them once the window is open.
    pub fn take_window_requests(&mut self) -> Vec<(WindowId, bool)> {
        std::mem::take(&mut self.window_requests)
    }

    pub fn theme(&self) -> &ThemeSpec {
        &self.theme
    }

    pub fn shortcuts(&self) -> &[String] {
        &self.shortcuts
    }

    pub fn focus_request(&self) -> Option<NodeId> {
        self.focus_request
    }

    pub fn clear_focus_request(&mut self) {
        self.focus_request = None;
    }

    pub fn tray(&self) -> Option<&TraySpec> {
        self.tray.as_ref()
    }

    /// Whether the app has nothing left on screen after the last commit removed the tray.
    pub fn tray_removed_and_idle(&self) -> bool {
        self.tray_removed && self.tray.is_none() && !self.has_open_window()
    }

    /// A window counts as open once it has a root.
    pub fn has_open_window(&self) -> bool {
        self.windows.values().any(|spec| spec.root.is_some())
    }

    pub fn commits(&self) -> u64 {
        self.commits
    }

    /// Forgets a window; `false` if it did not exist.
    pub fn close_window(&mut self, id: WindowId) -> bool {
        self.windows.remove(&id).is_some()
    }

    /// The node and all its descendants.
    pub fn subtree(&self, root: NodeId) -> HashSet<NodeId> {
        let mut ids = HashSet::new();
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            if let Some(node) = self.nodes.get(&id).filter(|_| ids.insert(id)) {
                stack.extend(&node.children);
            }
        }
        ids
    }

    /// Whether the node subscribed to `event` (events are opt-in).
    pub fn wants(&self, id: NodeId, event: &str) -> bool {
        self.nodes
            .get(&id)
            .is_some_and(|node| node.events.contains(event))
    }

    /// Queues an operation until the next `commit`.
    pub fn queue(&mut self, op: ClientMessage) {
        self.pending.push(op);
    }

    /// Applies queued operations in order. A failing operation is skipped and
    /// reported; the rest of the batch still applies.
    pub fn commit(&mut self) -> Vec<ProtocolError> {
        self.commits += 1;
        self.value_writes.clear();
        self.tray_removed = false;
        // Reloads and `minimized` requests not taken since the previous commit had no window
        // to apply them (or the host runs with `--no-window`).
        self.image_reloads.clear();
        self.window_requests.clear();
        std::mem::take(&mut self.pending)
            .into_iter()
            .filter_map(|op| self.apply(op).err())
            .collect()
    }

    /// Input nodes whose content the last commit replaced, even if `value` did not change
    /// (clearing a field twice with `""` must work both times). Each window applies them once per commit.
    pub fn value_writes(&self) -> &[NodeId] {
        &self.value_writes
    }

    /// Image files to drop from GPUI's cache, which keeps an image by path even after the file changes.
    pub fn take_image_reloads(&mut self) -> Vec<PathBuf> {
        std::mem::take(&mut self.image_reloads)
    }

    /// The file that holds an `Image` node's `data` bytes, while it has a non-empty `data`.
    pub fn image_path(&self, id: NodeId) -> Option<PathBuf> {
        let data = self
            .nodes
            .get(&id)?
            .props
            .get("data")
            .and_then(Value::as_str)?;
        if data.is_empty() {
            return None;
        }
        self.image_data.path(id)
    }

    /// Updates a prop from host-side interaction (e.g. a checkbox toggled by the user).
    pub fn set_host_prop(&mut self, id: NodeId, key: &str, value: Value) {
        if let Some(node) = self.nodes.get_mut(&id) {
            node.props.insert(key.into(), value);
        }
    }

    /// Rejects known props with a wrong type; unknown props only warn (once per type and name).
    fn check_props(&mut self, node_type: NodeType, props: &Props) -> Result<(), String> {
        for (key, value) in props {
            match node_type.prop_kind(key) {
                _ if value.is_null() => {}
                Some(kind) if !kind.accepts(value) => {
                    let reason = kind.check(value).err().unwrap_or_default();
                    return Err(format!(
                        "Prop '{key}' of {} must be {}, got {value}",
                        node_type.name(),
                        reason
                    ));
                }
                // Not an error (the file may appear later), but GPUI's own load error omits the path.
                // A node with bytes (`data`) does not use its `src`, so a missing one is not worth a warning.
                Some(_) if node_type == NodeType::Image && key == "src" => {
                    let src = value.as_str().unwrap_or_default();
                    let bytes = props
                        .get("data")
                        .and_then(Value::as_str)
                        .is_some_and(|data| !data.is_empty());
                    if !bytes && !src.is_empty() && !std::path::Path::new(src).is_file() {
                        log::warn!("Image src not found: {src}");
                    }
                }
                Some(_) => {}
                None => {
                    if self.warned_props.insert((node_type, key.clone())) {
                        log::warn!("unknown prop '{key}' on {} ignored", node_type.name());
                    }
                }
            }
        }
        Ok(())
    }

    fn track_image_src(&mut self, node_type: NodeType, props: &Props) {
        if let (NodeType::Image, Some(src)) = (node_type, props.get("src").and_then(Value::as_str))
        {
            self.image_reloads.push(PathBuf::from(src));
        }
    }

    /// Decodes an `Image` node's `data` into its own file, or removes that file when `data` is
    /// `null` or empty. The value is checked to be a string before this runs; base64 is checked
    /// here so an invalid payload never reaches the error message (it can be megabytes long).
    fn store_image_data(&mut self, id: NodeId, props: &Props) -> Result<(), String> {
        let Some(value) = props.get("data") else {
            return Ok(());
        };
        if value.is_null() {
            self.image_data.remove(id);
            return Ok(());
        }
        let encoded = value.as_str().unwrap_or_default();
        if encoded.is_empty() {
            self.image_data.remove(id);
            return Ok(());
        }
        let bytes = image_data::decode(encoded)
            .map_err(|_| "Prop 'data' of Image must be standard base64".to_string())?;
        match self.image_data.write(id, &bytes) {
            Ok(path) => self.image_reloads.push(path),
            // Not a protocol error: the node stays valid and shows nothing, like a missing file.
            Err(err) => log::error!("could not store Image data for node {id}: {err}"),
        }
        Ok(())
    }

    fn apply(&mut self, op: ClientMessage) -> Result<(), ProtocolError> {
        let op_name = op.op_name();
        let fail = |code, message: String| ProtocolError {
            code,
            message,
            op: Some(op_name.into()),
        };

        match op {
            ClientMessage::Window {
                id,
                title,
                width,
                height,
                x,
                y,
                min_width,
                min_height,
                root,
                theme,
                theme_set,
                frameless,
                minimized,
                events,
            } => {
                let id = id.unwrap_or(1);
                if id == 0 {
                    return Err(fail(
                        ErrorCode::InvalidOperation,
                        "Window ids must be positive".into(),
                    ));
                }
                if let Some(root) = root
                    && let Some(other) = self
                        .windows
                        .iter()
                        .find(|&(&w, spec)| w != id && spec.root == Some(root))
                {
                    return Err(fail(
                        ErrorCode::InvalidOperation,
                        format!("Node {root} is already the root of window {}", other.0),
                    ));
                }
                if let Some(events) = &events
                    && let Some(bad) = events
                        .iter()
                        .find(|event| !WINDOW_EVENTS.contains(&event.as_str()))
                {
                    return Err(fail(
                        ErrorCode::InvalidJson,
                        format!(
                            "Unknown window event '{bad}' (expected one of {})",
                            WINDOW_EVENTS.join(", ")
                        ),
                    ));
                }
                {
                    let window = self.windows.entry(id).or_default();
                    window.title = title.or(window.title.take());
                    window.width = width.or(window.width);
                    window.height = height.or(window.height);
                    window.x = x.or(window.x);
                    window.y = y.or(window.y);
                    window.min_width = min_width.or(window.min_width);
                    window.min_height = min_height.or(window.min_height);
                    window.root = root.or(window.root);
                    window.frameless = frameless.or(window.frameless);
                    if let Some(events) = events {
                        window.events = events.into_iter().collect();
                    }
                }
                if let Some(minimized) = minimized {
                    self.window_requests.push((id, minimized));
                }
                self.theme.theme = theme.or(self.theme.theme);
                self.theme.theme_set = theme_set.or(self.theme.theme_set.take());
            }
            ClientMessage::CloseWindow { id } => {
                if !self.close_window(id) {
                    return Err(fail(
                        ErrorCode::InvalidOperation,
                        format!("Window {id} does not exist"),
                    ));
                }
            }
            ClientMessage::Tray {
                icon,
                tooltip,
                menu,
            } => {
                if let Some(menu) = &menu {
                    crate::components::menu::check_items(menu).map_err(|err| {
                        fail(
                            ErrorCode::InvalidJson,
                            format!("'menu' must be a menu item list ({err})"),
                        )
                    })?;
                }
                if let Some(icon) = icon
                    .as_deref()
                    .filter(|icon| !std::path::Path::new(icon).is_file())
                {
                    log::warn!("Tray icon not found: {icon}");
                }
                let tray = self.tray.get_or_insert_default();
                tray.icon = icon.or(tray.icon.take());
                tray.tooltip = tooltip.or(tray.tooltip.take());
                tray.menu = menu.or(tray.menu.take());
                tray.revision += 1;
            }
            ClientMessage::Shortcuts { keys } => {
                if let Some(bad) = keys.iter().find(|keys| crate::keys::parse(keys).is_none()) {
                    return Err(fail(
                        ErrorCode::InvalidJson,
                        format!("Invalid shortcut '{bad}'"),
                    ));
                }
                self.shortcuts = keys;
            }
            ClientMessage::Focus { id } => {
                let node = self.nodes.get(&id).ok_or_else(|| {
                    fail(ErrorCode::UnknownNode, format!("Node {id} does not exist"))
                })?;
                if !matches!(
                    node.node_type,
                    NodeType::Input | NodeType::Textarea | NodeType::Div | NodeType::Table
                ) {
                    return Err(fail(
                        ErrorCode::InvalidOperation,
                        format!("A {} cannot take the focus", node.node_type.name()),
                    ));
                }
                self.focus_request = Some(id);
            }
            ClientMessage::RemoveTray => {
                if self.tray.take().is_none() {
                    return Err(fail(
                        ErrorCode::InvalidOperation,
                        "There is no tray to remove".into(),
                    ));
                }
                self.tray_removed = true;
            }
            ClientMessage::Create {
                id,
                node_type,
                props,
                events,
            } => {
                if id == 0 {
                    return Err(fail(
                        ErrorCode::InvalidOperation,
                        "Node ids must be positive".into(),
                    ));
                }
                if self.nodes.contains_key(&id) {
                    return Err(fail(
                        ErrorCode::InvalidOperation,
                        format!("Node {id} already exists"),
                    ));
                }
                let node_type = NodeType::parse(&node_type).ok_or_else(|| {
                    fail(
                        ErrorCode::UnknownType,
                        format!("Unknown node type '{node_type}'"),
                    )
                })?;
                self.check_props(node_type, &props)
                    .map_err(|msg| fail(ErrorCode::InvalidProps, msg))?;
                node_type
                    .check_required(&props)
                    .map_err(|msg| fail(ErrorCode::InvalidProps, msg))?;
                if matches!(node_type, NodeType::Input | NodeType::Textarea)
                    && props.get("value").is_some_and(|v| !v.is_null())
                {
                    self.value_writes.push(id);
                }
                self.track_image_src(node_type, &props);
                if node_type == NodeType::Image {
                    self.store_image_data(id, &props)
                        .map_err(|msg| fail(ErrorCode::InvalidProps, msg))?;
                }
                let props = props
                    .into_iter()
                    .filter(|(_, value)| !value.is_null())
                    .collect();
                let events = events.into_iter().collect();
                self.nodes.insert(
                    id,
                    Node {
                        node_type,
                        props,
                        events,
                        children: vec![],
                        parent: None,
                    },
                );
            }
            ClientMessage::Append { parent, child } => {
                self.check_attach(parent, child)
                    .map_err(|(code, msg)| fail(code, msg))?;
                self.detach(child);
                self.insert_child(parent, child, None);
            }
            ClientMessage::InsertBefore {
                parent,
                child,
                before,
            } => {
                self.check_attach(parent, child)
                    .map_err(|(code, msg)| fail(code, msg))?;
                if !self.nodes.contains_key(&before) {
                    return Err(fail(
                        ErrorCode::UnknownNode,
                        format!("Node {before} does not exist"),
                    ));
                }
                if child == before || self.nodes[&before].parent != Some(parent) {
                    return Err(fail(
                        ErrorCode::InvalidOperation,
                        format!("Node {before} is not a child of node {parent}"),
                    ));
                }
                self.detach(child);
                self.insert_child(parent, child, Some(before));
            }
            ClientMessage::SetProps { id, props } => {
                let node_type = self
                    .node_mut(id)
                    .map_err(|msg| fail(ErrorCode::UnknownNode, msg))?
                    .node_type;
                self.check_props(node_type, &props)
                    .map_err(|msg| fail(ErrorCode::InvalidProps, msg))?;
                // Only when the op touches it: a `Tab` keeps the `value` it was created with.
                if props.contains_key("value") {
                    node_type
                        .check_required(&props)
                        .map_err(|msg| fail(ErrorCode::InvalidProps, msg))?;
                }
                if matches!(node_type, NodeType::Input | NodeType::Textarea)
                    && props.contains_key("value")
                {
                    self.value_writes.push(id);
                }
                self.track_image_src(node_type, &props);
                if node_type == NodeType::Image {
                    self.store_image_data(id, &props)
                        .map_err(|msg| fail(ErrorCode::InvalidProps, msg))?;
                }
                let node = self.node_mut(id).expect("checked above");
                for (key, value) in props {
                    if value.is_null() {
                        node.props.remove(&key);
                    } else {
                        node.props.insert(key, value);
                    }
                }
            }
            ClientMessage::SetEvents { id, events } => {
                let node = self
                    .node_mut(id)
                    .map_err(|msg| fail(ErrorCode::UnknownNode, msg))?;
                node.events = events.into_iter().collect();
            }
            ClientMessage::Remove { id } => {
                if !self.nodes.contains_key(&id) {
                    return Err(fail(
                        ErrorCode::UnknownNode,
                        format!("Node {id} does not exist"),
                    ));
                }
                self.detach(id);
                let mut stack = vec![id];
                while let Some(id) = stack.pop() {
                    if let Some(node) = self.nodes.remove(&id) {
                        if node.node_type == NodeType::Image {
                            self.image_data.remove(id);
                        }
                        stack.extend(node.children);
                    }
                }
            }
            ClientMessage::Hello { .. }
            | ClientMessage::Commit
            | ClientMessage::OpenDialog { .. }
            | ClientMessage::SaveDialog { .. }
            | ClientMessage::ReadClipboard { .. }
            | ClientMessage::WriteClipboard { .. }
            | ClientMessage::OpenUrl { .. }
            | ClientMessage::OpenPath { .. }
            | ClientMessage::Notify { .. } => {
                unreachable!("{op_name} is handled by the session, never queued")
            }
        }
        Ok(())
    }

    fn node_mut(&mut self, id: NodeId) -> Result<&mut Node, String> {
        self.nodes
            .get_mut(&id)
            .ok_or_else(|| format!("Node {id} does not exist"))
    }

    /// Both nodes must exist and `child` must not be `parent` or one of its ancestors.
    fn check_attach(&self, parent: NodeId, child: NodeId) -> Result<(), (ErrorCode, String)> {
        for id in [parent, child] {
            if !self.nodes.contains_key(&id) {
                return Err((ErrorCode::UnknownNode, format!("Node {id} does not exist")));
            }
        }
        let mut current = Some(parent);
        while let Some(id) = current {
            if id == child {
                return Err((
                    ErrorCode::InvalidOperation,
                    format!(
                        "Node {child} cannot be placed inside itself or its descendant {parent}"
                    ),
                ));
            }
            current = self.nodes[&id].parent;
        }
        Ok(())
    }

    fn detach(&mut self, child: NodeId) {
        if let Some(parent) = self
            .nodes
            .get_mut(&child)
            .and_then(|node| node.parent.take())
            && let Some(parent) = self.nodes.get_mut(&parent)
        {
            parent.children.retain(|&id| id != child);
        }
    }

    fn insert_child(&mut self, parent: NodeId, child: NodeId, before: Option<NodeId>) {
        let children = &mut self
            .nodes
            .get_mut(&parent)
            .expect("parent checked")
            .children;
        let index = before
            .and_then(|before| children.iter().position(|&id| id == before))
            .unwrap_or(children.len());
        children.insert(index, child);
        self.nodes.get_mut(&child).expect("child checked").parent = Some(parent);
    }

    /// Human-readable dump for stderr: the window, then every top-level node (root first).
    pub fn dump(&self) -> String {
        let mut out = String::new();
        if let Some(theme) = self.theme.theme {
            let _ = writeln!(out, "theme={theme:?}");
        }
        if let Some(CustomThemes(set)) = &self.theme.theme_set {
            let _ = writeln!(out, "themeSet={:?}", set.name);
        }
        for (id, w) in &self.windows {
            let _ = write!(
                out,
                "window {id} title={:?} size={}x{} root={}",
                w.title.as_deref().unwrap_or(""),
                w.width.map_or("-".into(), |v| v.to_string()),
                w.height.map_or("-".into(), |v| v.to_string()),
                w.root.map_or("-".into(), |v| v.to_string()),
            );
            if w.frameless == Some(true) {
                out.push_str(" frameless");
            }
            if w.x.is_some() || w.y.is_some() {
                let _ = write!(
                    out,
                    " pos={}x{}",
                    w.x.map_or("-".into(), |v| v.to_string()),
                    w.y.map_or("-".into(), |v| v.to_string())
                );
            }
            if w.min_width.is_some() || w.min_height.is_some() {
                let _ = write!(
                    out,
                    " min={}x{}",
                    w.min_width.map_or("-".into(), |v| v.to_string()),
                    w.min_height.map_or("-".into(), |v| v.to_string())
                );
            }
            if !w.events.is_empty() {
                let events: Vec<&str> = w.events.iter().map(String::as_str).collect();
                let _ = write!(out, " events=[{}]", events.join(","));
            }
            out.push('\n');
        }
        let roots: HashSet<NodeId> = self.windows.values().filter_map(|w| w.root).collect();
        let mut top: Vec<NodeId> = self
            .nodes
            .iter()
            .filter(|(_, n)| n.parent.is_none())
            .map(|(&id, _)| id)
            .collect();
        top.sort_by_key(|&id| (!roots.contains(&id), id));
        for id in top {
            self.dump_node(&mut out, id, 1);
        }
        out
    }

    fn dump_node(&self, out: &mut String, id: NodeId, depth: usize) {
        let node = &self.nodes[&id];
        let _ = write!(out, "{}#{id} {}", "  ".repeat(depth), node.node_type.name());
        if !node.props.is_empty() {
            let _ = write!(out, " {}", serde_json::Value::Object(node.props.clone()));
        }
        if !node.events.is_empty() {
            let events: Vec<&str> = node.events.iter().map(String::as_str).collect();
            let _ = write!(out, " events=[{}]", events.join(","));
        }
        out.push('\n');
        for &child in &node.children {
            self.dump_node(out, child, depth + 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn op(value: serde_json::Value) -> ClientMessage {
        crate::protocol::parse_client_message(&value.to_string()).unwrap()
    }

    /// Queues the operations, commits, and returns the error codes.
    fn run(tree: &mut Tree, ops: Vec<serde_json::Value>) -> Vec<ErrorCode> {
        for value in ops {
            tree.queue(op(value));
        }
        tree.commit().into_iter().map(|err| err.code).collect()
    }

    fn create(id: NodeId, node_type: &str) -> serde_json::Value {
        json!({"op": "create", "id": id, "type": node_type})
    }

    fn append(parent: NodeId, child: NodeId) -> serde_json::Value {
        json!({"op": "append", "parent": parent, "child": child})
    }

    fn children(tree: &Tree, id: NodeId) -> Vec<NodeId> {
        tree.node(id).unwrap().children.clone()
    }

    #[test]
    fn operations_apply_only_on_commit() {
        let mut tree = Tree::default();
        tree.queue(op(create(1, "div")));
        assert!(tree.node(1).is_none());
        assert!(tree.commit().is_empty());
        assert!(tree.node(1).is_some());
    }

    #[test]
    fn create_validates_id_and_type() {
        let mut tree = Tree::default();
        assert_eq!(run(&mut tree, vec![create(1, "div")]), vec![]);
        assert_eq!(
            run(&mut tree, vec![create(1, "text")]),
            vec![ErrorCode::InvalidOperation]
        );
        assert_eq!(
            run(&mut tree, vec![create(0, "div")]),
            vec![ErrorCode::InvalidOperation]
        );
        assert_eq!(
            run(&mut tree, vec![create(2, "span")]),
            vec![ErrorCode::UnknownType]
        );
        assert_eq!(tree.node(1).unwrap().node_type, NodeType::Div);
    }

    #[test]
    fn append_links_and_moves_children() {
        let mut tree = Tree::default();
        let errors = run(
            &mut tree,
            vec![
                create(1, "div"),
                create(2, "div"),
                create(3, "text"),
                append(1, 3),
                append(2, 3),
            ],
        );
        assert_eq!(errors, vec![]);
        assert_eq!(children(&tree, 1), Vec::<NodeId>::new());
        assert_eq!(children(&tree, 2), vec![3]);
        assert_eq!(tree.node(3).unwrap().parent, Some(2));
    }

    #[test]
    fn append_rejects_missing_nodes_and_cycles() {
        let mut tree = Tree::default();
        run(
            &mut tree,
            vec![create(1, "div"), create(2, "div"), append(1, 2)],
        );
        assert_eq!(
            run(&mut tree, vec![append(9, 1)]),
            vec![ErrorCode::UnknownNode]
        );
        assert_eq!(
            run(&mut tree, vec![append(1, 9)]),
            vec![ErrorCode::UnknownNode]
        );
        assert_eq!(
            run(&mut tree, vec![append(2, 1)]),
            vec![ErrorCode::InvalidOperation]
        );
        assert_eq!(
            run(&mut tree, vec![append(1, 1)]),
            vec![ErrorCode::InvalidOperation]
        );
        assert_eq!(children(&tree, 1), vec![2]);
    }

    #[test]
    fn insert_before_orders_children() {
        let mut tree = Tree::default();
        let insert = |child: NodeId, before: NodeId| json!({"op": "insertBefore", "parent": 1, "child": child, "before": before});
        let errors = run(
            &mut tree,
            vec![
                create(1, "div"),
                create(2, "text"),
                create(3, "text"),
                create(4, "text"),
                append(1, 2),
                insert(3, 2),
                insert(4, 2),
            ],
        );
        assert_eq!(errors, vec![]);
        assert_eq!(children(&tree, 1), vec![3, 4, 2]);

        // Moving an existing child within the same parent.
        assert_eq!(run(&mut tree, vec![insert(2, 3)]), vec![]);
        assert_eq!(children(&tree, 1), vec![2, 3, 4]);

        assert_eq!(
            run(&mut tree, vec![create(5, "text"), insert(5, 9)]),
            vec![ErrorCode::UnknownNode]
        );
        assert_eq!(
            run(&mut tree, vec![create(6, "div"), insert(5, 6)]),
            vec![ErrorCode::InvalidOperation]
        );
        assert_eq!(
            run(&mut tree, vec![insert(3, 3)]),
            vec![ErrorCode::InvalidOperation]
        );
        assert_eq!(children(&tree, 1), vec![2, 3, 4]);
    }

    #[test]
    fn set_props_merges_and_removes_nulls() {
        let mut tree = Tree::default();
        run(
            &mut tree,
            vec![
                json!({"op": "create", "id": 1, "type": "Button", "props": {"label": "Save", "variant": "primary", "class": null}}),
            ],
        );
        assert_eq!(
            serde_json::Value::Object(tree.node(1).unwrap().props.clone()),
            json!({"label": "Save", "variant": "primary"})
        );

        let errors = run(
            &mut tree,
            vec![json!({"op": "setProps", "id": 1, "props": {"disabled": true, "variant": null}})],
        );
        assert_eq!(errors, vec![]);
        assert_eq!(
            serde_json::Value::Object(tree.node(1).unwrap().props.clone()),
            json!({"label": "Save", "disabled": true})
        );

        assert_eq!(
            run(
                &mut tree,
                vec![json!({"op": "setProps", "id": 9, "props": {}})]
            ),
            vec![ErrorCode::UnknownNode]
        );
    }

    #[test]
    fn set_events_replaces_subscriptions() {
        let mut tree = Tree::default();
        run(
            &mut tree,
            vec![json!({"op": "create", "id": 1, "type": "Input", "events": ["change"]})],
        );
        assert_eq!(
            run(
                &mut tree,
                vec![json!({"op": "setEvents", "id": 1, "events": ["submit"]})]
            ),
            vec![]
        );
        assert_eq!(
            tree.node(1).unwrap().events,
            BTreeSet::from(["submit".to_string()])
        );
        assert_eq!(
            run(
                &mut tree,
                vec![json!({"op": "setEvents", "id": 9, "events": []})]
            ),
            vec![ErrorCode::UnknownNode]
        );
    }

    #[test]
    fn remove_deletes_descendants_and_detaches() {
        let mut tree = Tree::default();
        run(
            &mut tree,
            vec![
                create(1, "div"),
                create(2, "div"),
                create(3, "text"),
                create(4, "text"),
                append(1, 2),
                append(2, 3),
                append(1, 4),
            ],
        );
        assert_eq!(
            run(&mut tree, vec![json!({"op": "remove", "id": 2})]),
            vec![]
        );
        assert!(tree.node(2).is_none());
        assert!(tree.node(3).is_none());
        assert_eq!(children(&tree, 1), vec![4]);
        assert_eq!(
            run(&mut tree, vec![json!({"op": "remove", "id": 2})]),
            vec![ErrorCode::UnknownNode]
        );
    }

    #[test]
    fn failing_op_does_not_stop_the_batch() {
        let mut tree = Tree::default();
        let errors = run(
            &mut tree,
            vec![
                create(1, "div"),
                append(1, 9),
                create(2, "text"),
                append(1, 2),
            ],
        );
        assert_eq!(errors, vec![ErrorCode::UnknownNode]);
        assert_eq!(children(&tree, 1), vec![2]);
    }

    #[test]
    fn props_are_validated_per_type() {
        let mut tree = Tree::default();
        let button = |props: serde_json::Value| json!({"op": "create", "id": 1, "type": "Button", "props": props});
        assert_eq!(
            run(&mut tree, vec![button(json!({"disabled": "yes"}))]),
            vec![ErrorCode::InvalidProps]
        );
        assert_eq!(
            run(&mut tree, vec![button(json!({"variant": "huge"}))]),
            vec![ErrorCode::InvalidProps]
        );
        assert!(
            tree.node(1).is_none(),
            "an invalid create must not create the node"
        );

        // Valid, null and unknown props are accepted.
        assert_eq!(
            run(
                &mut tree,
                vec![button(
                    json!({"label": "Go", "variant": "danger", "class": null, "tooltip": "x"})
                )]
            ),
            vec![]
        );

        let set = |props: serde_json::Value| json!({"op": "setProps", "id": 1, "props": props});
        assert_eq!(
            run(&mut tree, vec![set(json!({"label": 3, "disabled": true}))]),
            vec![ErrorCode::InvalidProps]
        );
        assert_eq!(
            tree.node(1).unwrap().props.get("disabled"),
            None,
            "an invalid setProps applies nothing"
        );

        run(
            &mut tree,
            vec![
                json!({"op": "create", "id": 2, "type": "Checkbox"}),
                json!({"op": "create", "id": 3, "type": "text"}),
            ],
        );
        assert_eq!(
            run(
                &mut tree,
                vec![json!({"op": "setProps", "id": 2, "props": {"checked": "true"}})]
            ),
            vec![ErrorCode::InvalidProps]
        );
        assert_eq!(
            run(
                &mut tree,
                vec![json!({"op": "setProps", "id": 2, "props": {"controlled": 1}})]
            ),
            vec![ErrorCode::InvalidProps]
        );
        assert_eq!(
            run(
                &mut tree,
                vec![json!({"op": "setProps", "id": 2, "props": {"controlled": true}})]
            ),
            vec![]
        );
        assert_eq!(
            run(
                &mut tree,
                vec![json!({"op": "setProps", "id": 3, "props": {"content": 5}})]
            ),
            vec![ErrorCode::InvalidProps]
        );
    }

    #[test]
    fn image_src_is_reloaded_each_time_it_is_sent() {
        let mut tree = Tree::default();
        run(
            &mut tree,
            vec![json!({"op": "create", "id": 1, "type": "Image", "props": {"src": "a.png"}})],
        );
        assert_eq!(tree.take_image_reloads(), vec![PathBuf::from("a.png")]);
        // The same path again still reloads (the file may have changed); other props do not.
        run(
            &mut tree,
            vec![json!({"op": "setProps", "id": 1, "props": {"src": "a.png"}})],
        );
        assert_eq!(tree.take_image_reloads(), vec![PathBuf::from("a.png")]);
        run(
            &mut tree,
            vec![json!({"op": "setProps", "id": 1, "props": {"fit": "cover"}})],
        );
        assert_eq!(tree.take_image_reloads(), Vec::<PathBuf>::new());
    }

    #[test]
    fn image_data_is_written_to_a_file_per_node_and_removed() {
        let mut tree = Tree::default();
        // `aGVsbG8=` is "hello"; a valid image is not needed to store the bytes.
        run(
            &mut tree,
            vec![json!({"op": "create", "id": 1, "type": "Image", "props": {"data": "aGVsbG8="}})],
        );
        let path = tree.image_path(1).expect("the node has a file");
        assert_eq!(std::fs::read(&path).unwrap(), b"hello");
        assert_eq!(tree.take_image_reloads(), vec![path.clone()]);

        // Sending bytes again rewrites the same file and asks GPUI to read it again.
        run(
            &mut tree,
            vec![json!({"op": "setProps", "id": 1, "props": {"data": "d29ybGQ="}})],
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"world");
        assert_eq!(tree.take_image_reloads(), vec![path.clone()]);

        // Empty or null `data` goes back to `src` (if any) and removes the file.
        run(
            &mut tree,
            vec![json!({"op": "setProps", "id": 1, "props": {"data": null}})],
        );
        assert!(!path.exists());
        assert_eq!(tree.image_path(1), None);
        assert_eq!(tree.take_image_reloads(), Vec::<PathBuf>::new());

        // Removing the node removes its file too.
        run(
            &mut tree,
            vec![json!({"op": "setProps", "id": 1, "props": {"data": "aGVsbG8="}})],
        );
        let path = tree.image_path(1).expect("the node has a file again");
        run(&mut tree, vec![json!({"op": "remove", "id": 1})]);
        assert!(!path.exists());
    }

    #[test]
    fn image_data_files_go_away_with_the_tree() {
        let dir = {
            let mut tree = Tree::default();
            run(
                &mut tree,
                vec![
                    json!({"op": "create", "id": 1, "type": "Image", "props": {"data": "aGVsbG8="}}),
                ],
            );
            tree.image_path(1).unwrap().parent().unwrap().to_path_buf()
        };
        assert!(
            !dir.exists(),
            "the tree removes its temp directory when dropped"
        );
    }

    #[test]
    fn image_data_must_be_base64() {
        let mut tree = Tree::default();
        let errors = run(
            &mut tree,
            vec![
                json!({"op": "create", "id": 1, "type": "Image", "props": {"data": "not base64!"}}),
            ],
        );
        assert_eq!(errors, vec![ErrorCode::InvalidProps]);
        assert!(tree.node(1).is_none(), "a rejected create applies nothing");

        // A non-string is rejected by the prop check; the node keeps its previous bytes.
        run(
            &mut tree,
            vec![json!({"op": "create", "id": 1, "type": "Image", "props": {"data": "aGVsbG8="}})],
        );
        let errors = run(
            &mut tree,
            vec![json!({"op": "setProps", "id": 1, "props": {"data": 42}})],
        );
        assert_eq!(errors, vec![ErrorCode::InvalidProps]);
        assert_eq!(
            std::fs::read(tree.image_path(1).unwrap()).unwrap(),
            b"hello"
        );
    }

    #[test]
    fn input_value_writes_are_tracked_per_commit() {
        let mut tree = Tree::default();
        let set_value = || json!({"op": "setProps", "id": 1, "props": {"value": ""}});
        run(
            &mut tree,
            vec![json!({"op": "create", "id": 1, "type": "Input", "props": {"value": "hi"}})],
        );
        assert_eq!((tree.value_writes(), tree.commits()), (&[1][..], 1));

        // The same value imposed twice is a write both times; each commit has its own number.
        run(&mut tree, vec![set_value()]);
        assert_eq!((tree.value_writes(), tree.commits()), (&[1][..], 2));
        run(&mut tree, vec![set_value()]);
        assert_eq!((tree.value_writes(), tree.commits()), (&[1][..], 3));

        // Other props are not value writes; the next commit drops the previous writes.
        run(
            &mut tree,
            vec![json!({"op": "setProps", "id": 1, "props": {"placeholder": "Type"}})],
        );
        assert_eq!(tree.value_writes(), &[] as &[NodeId]);
        run(&mut tree, vec![set_value()]);
        run(&mut tree, vec![]);
        assert_eq!(tree.value_writes(), &[] as &[NodeId]);
    }

    #[test]
    fn window_updates_merge() {
        let mut tree = Tree::default();
        run(
            &mut tree,
            vec![
                json!({"op": "window", "title": "App", "width": 400, "height": 300, "root": 1, "theme": "dark"}),
            ],
        );
        run(&mut tree, vec![json!({"op": "window", "title": "Renamed"})]);
        assert_eq!(
            tree.windows().get(&1),
            Some(&WindowSpec {
                title: Some("Renamed".into()),
                width: Some(400.0),
                height: Some(300.0),
                root: Some(1),
                ..WindowSpec::default()
            })
        );
        assert_eq!(
            tree.theme(),
            &ThemeSpec {
                theme: Some(WindowTheme::Dark),
                theme_set: None
            }
        );
    }

    #[test]
    fn window_events_and_minimize_requests() {
        let mut tree = Tree::default();
        let errors = run(
            &mut tree,
            vec![
                json!({"op": "window", "root": 1, "events": ["focus", "resize"]}),
                json!({"op": "window", "events": ["hover"]}),
                json!({"op": "window", "minimized": true}),
            ],
        );
        assert_eq!(errors, [ErrorCode::InvalidJson]);
        assert!(tree.wants_window_event(1, "focus"));
        assert!(tree.wants_window_event(1, "resize"));
        assert!(!tree.wants_window_event(1, "visibility"));
        // `minimized` is an action: each message queues one, and the host drains them once.
        assert_eq!(tree.take_window_requests(), [(1, true)]);
        assert!(tree.take_window_requests().is_empty());

        // A new list replaces the previous one; an empty list unsubscribes.
        run(&mut tree, vec![json!({"op": "window", "events": ["blur"]})]);
        assert!(!tree.wants_window_event(1, "focus"));
        assert!(tree.wants_window_event(1, "blur"));
        run(&mut tree, vec![json!({"op": "window", "events": []})]);
        assert!(!tree.wants_window_event(1, "blur"));
    }

    #[test]
    fn windows_have_ids_and_their_own_roots() {
        let mut tree = Tree::default();
        let errors = run(
            &mut tree,
            vec![
                json!({"op": "window", "root": 1}),
                json!({"op": "window", "id": 2, "title": "Settings", "root": 2}),
                json!({"op": "window", "id": 3, "root": 1}),
                json!({"op": "window", "id": 0, "root": 5}),
                json!({"op": "closeWindow", "id": 9}),
                // The theme is app-wide, whichever window message carries it.
                json!({"op": "window", "id": 2, "theme": "dark"}),
            ],
        );
        assert_eq!(errors, [ErrorCode::InvalidOperation; 3]);
        assert_eq!(tree.windows().keys().copied().collect::<Vec<_>>(), [1, 2]);
        assert_eq!(tree.windows()[&2].title.as_deref(), Some("Settings"));
        assert_eq!(tree.theme().theme, Some(WindowTheme::Dark));

        run(&mut tree, vec![json!({"op": "closeWindow", "id": 1})]);
        assert_eq!(tree.windows().keys().copied().collect::<Vec<_>>(), [2]);
        // A closed window's root can be used again.
        assert!(run(&mut tree, vec![json!({"op": "window", "id": 3, "root": 1})]).is_empty());
    }

    #[test]
    fn dump_shows_hierarchy() {
        let mut tree = Tree::default();
        run(
            &mut tree,
            vec![
                json!({"op": "window", "title": "App", "width": 400, "height": 300, "root": 1}),
                create(1, "div"),
                json!({"op": "create", "id": 2, "type": "Button", "props": {"label": "Go"}, "events": ["click"]}),
                append(1, 2),
                create(3, "text"),
            ],
        );
        assert_eq!(
            tree.dump(),
            "window 1 title=\"App\" size=400x300 root=1\n  #1 div\n    #2 Button {\"label\":\"Go\"} events=[click]\n  #3 text\n"
        );
    }
}
