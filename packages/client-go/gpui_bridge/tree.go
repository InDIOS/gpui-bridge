package gpui_bridge

import "sort"

// nodeRecord is the client's local copy of a node in the host's tree.
type nodeRecord struct {
	typ      string
	parent   *NodeId
	children map[NodeId]bool
	handlers map[string]func(any)
}

// Tree is the client's bookkeeping: id allocation, pending operations and each node's
// event handlers.
type Tree struct {
	nextId  NodeId
	pending []ClientMessage
	nodes   map[NodeId]*nodeRecord
}

func newTree() *Tree {
	return &Tree{
		nextId:  1,
		nodes:   make(map[NodeId]*nodeRecord),
		pending: nil,
	}
}

// create registers a node, gives it the next id and queues its create operation.
// Ids are positive, unique per session and never reused.
func (t *Tree) create(typ string, props Props, events []string) NodeId {
	id := t.nextId
	t.nextId++
	t.nodes[id] = &nodeRecord{
		typ:      typ,
		parent:   nil,
		children: make(map[NodeId]bool),
		handlers: make(map[string]func(any)),
	}
	t.pending = append(t.pending, createMsg(id, typ, props, events))
	return id
}

// attach moves `child` under `parent` in the local copy (it mirrors the host's move
// semantics so `remove` can drop the descendants' handlers).
func (t *Tree) attach(parent, child NodeId) {
	rec, ok := t.nodes[child]
	if !ok {
		return
	}
	if rec.parent != nil {
		if p, ok2 := t.nodes[*rec.parent]; ok2 {
			delete(p.children, child)
		}
	}
	rec.parent = &parent
	if p, ok := t.nodes[parent]; ok {
		p.children[child] = true
	}
}

// remove drops `id` and all its descendants from the local copy and queues remove.
func (t *Tree) remove(id NodeId) {
	rec, ok := t.nodes[id]
	if !ok {
		return
	}
	if rec.parent != nil {
		if p, ok2 := t.nodes[*rec.parent]; ok2 {
			delete(p.children, id)
		}
	}
	stack := []NodeId{id}
	for len(stack) > 0 {
		cur := stack[len(stack)-1]
		stack = stack[:len(stack)-1]
		node := t.nodes[cur]
		if node == nil {
			continue
		}
		for child := range node.children {
			stack = append(stack, child)
		}
		delete(t.nodes, cur)
	}
	t.pending = append(t.pending, removeMsg(id))
}

// setHandler sets or clears an event's handler and, if the set changed, queues
// setEvents with the node's full event list.
func (t *Tree) setHandler(id NodeId, event string, handler func(any)) {
	rec, ok := t.nodes[id]
	if !ok {
		return
	}
	had := rec.handlers[event] != nil
	if handler != nil {
		rec.handlers[event] = handler
	} else {
		delete(rec.handlers, event)
	}
	now := rec.handlers[event] != nil
	if had != now {
		evs := make([]string, 0, len(rec.handlers))
		for e := range rec.handlers {
			evs = append(evs, e)
		}
		sort.Strings(evs) // map order is random; the wire's is not
		t.pending = append(t.pending, setEventsMsg(id, evs))
	}
}

// handler returns the handler registered for one of the node's events, if any.
func (t *Tree) handler(id NodeId, event string) (func(any), bool) {
	rec, ok := t.nodes[id]
	if !ok {
		return nil, false
	}
	h, ok := rec.handlers[event]
	return h, ok
}

// commit sends every pending operation and then `commit`.
func (t *Tree) commit(send func(ClientMessage)) {
	for _, op := range t.pending {
		send(op)
	}
	t.pending = nil
	send(commitMsg())
}
