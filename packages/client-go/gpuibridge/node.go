package gpuibridge

// Node is a tree node, identified by its id. Its methods queue operations in the
// client's Tree; they reach the host on the next Commit().
type Node struct {
	app *App
	id  NodeID
	typ string
}

// ID returns the node's id.
func (n *Node) ID() NodeID { return n.id }

// Type returns the node type (PROTOCOL.md §5).
func (n *Node) Type() string { return n.typ }

// Append adds children to the node and queues one append per child.
func (n *Node) Append(children ...*Node) *Node {
	n.app.mu.Lock()
	defer n.app.mu.Unlock()
	for _, c := range children {
		n.app.tree.attach(n.id, c.id)
		n.app.tree.pending = append(n.app.tree.pending, appendMsg(n.id, c.id))
	}
	return n
}

// InsertBefore adds `child` before `before` inside the node.
func (n *Node) InsertBefore(child, before *Node) *Node {
	n.app.mu.Lock()
	defer n.app.mu.Unlock()
	n.app.tree.attach(n.id, child.id)
	n.app.tree.pending = append(n.app.tree.pending, insertBeforeMsg(n.id, child.id, before.id))
	return n
}

// Set merges props into the node and queues setProps.
func (n *Node) Set(props Props) *Node {
	n.app.mu.Lock()
	defer n.app.mu.Unlock()
	n.app.tree.pending = append(n.app.tree.pending, setPropsMsg(n.id, props))
	return n
}

// On registers a handler for one of the node's events; when the host reports it, the
// handler gets the event payload (a map without the type/id/event keys).
func (n *Node) On(event string, handler func(any)) *Node {
	n.app.mu.Lock()
	defer n.app.mu.Unlock()
	n.app.tree.setHandler(n.id, event, handler)
	return n
}

// Off unsubscribes from one of the node's events.
func (n *Node) Off(event string) *Node {
	n.app.mu.Lock()
	defer n.app.mu.Unlock()
	n.app.tree.setHandler(n.id, event, nil)
	return n
}

// Remove drops the node and its descendants and queues remove.
func (n *Node) Remove() {
	n.app.mu.Lock()
	defer n.app.mu.Unlock()
	n.app.tree.remove(n.id)
}
