package gpui_bridge

// nodeRecord es la copia local que el cliente mantiene de un nodo del árbol del host.
type nodeRecord struct {
	typ      string
	parent   *NodeId
	children map[NodeId]bool
	handlers map[string]func(any)
}

// Tree lleva la contabilidad del cliente: asignación de ids, operaciones pendientes
// y los handlers de eventos de cada nodo.
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

// create registra un nodo, le asigna el siguiente id y encola su operación create.
// Los ids son positivos, únicos por sesión y nunca se reutilizan.
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

// attach mueve `child` bajo `parent` en la copia local (espeja la semántica de
// movimiento del host para que `remove` pueda soltar los handlers de los descendientes).
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

// remove descarga `id` y todos sus descendientes de la copia local y encola remove.
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

// setHandler asigna o quita el handler de un evento y, si el conjunto cambió,
// encola setEvents con la lista completa de eventos del nodo.
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
		t.pending = append(t.pending, setEventsMsg(id, evs))
	}
}

// handler devuelve el handler registrado para un evento del nodo, si lo hay.
func (t *Tree) handler(id NodeId, event string) (func(any), bool) {
	rec, ok := t.nodes[id]
	if !ok {
		return nil, false
	}
	h, ok := rec.handlers[event]
	return h, ok
}

// commit envía todas las operaciones pendientes y luego `commit`.
func (t *Tree) commit(send func(ClientMessage)) {
	for _, op := range t.pending {
		send(op)
	}
	t.pending = nil
	send(commitMsg())
}
