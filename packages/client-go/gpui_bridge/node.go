package gpui_bridge

// Node es un nodo del árbol, identificado por su id. Los métodos encolan operaciones
// en el Tree del cliente; se envían al host en el próximo Commit().
type Node struct {
	app *App
	id  NodeId
	typ string
}

// Id devuelve el identificador del nodo.
func (n *Node) Id() NodeId { return n.id }

// Type devuelve el tipo de nodo (PROTOCOL.md §5).
func (n *Node) Type() string { return n.typ }

// Append añade hijos al nodo y encola una operación append por cada uno.
func (n *Node) Append(children ...*Node) *Node {
	n.app.mu.Lock()
	defer n.app.mu.Unlock()
	for _, c := range children {
		n.app.tree.attach(n.id, c.id)
		n.app.tree.pending = append(n.app.tree.pending, appendMsg(n.id, c.id))
	}
	return n
}

// InsertBefore añade `child` antes de `before` dentro del nodo.
func (n *Node) InsertBefore(child, before *Node) *Node {
	n.app.mu.Lock()
	defer n.app.mu.Unlock()
	n.app.tree.attach(n.id, child.id)
	n.app.tree.pending = append(n.app.tree.pending, insertBeforeMsg(n.id, child.id, before.id))
	return n
}

// Set combina props en el nodo y encola setProps.
func (n *Node) Set(props Props) *Node {
	n.app.mu.Lock()
	defer n.app.mu.Unlock()
	n.app.tree.pending = append(n.app.tree.pending, setPropsMsg(n.id, props))
	return n
}

// On registra un handler para un evento del nodo; cuando el host lo informe, se invoca
// con el payload del evento (un mapa sin las claves type/id/event).
func (n *Node) On(event string, handler func(any)) *Node {
	n.app.mu.Lock()
	defer n.app.mu.Unlock()
	n.app.tree.setHandler(n.id, event, handler)
	return n
}

// Off quita la suscripción a un evento del nodo.
func (n *Node) Off(event string) *Node {
	n.app.mu.Lock()
	defer n.app.mu.Unlock()
	n.app.tree.setHandler(n.id, event, nil)
	return n
}

// Remove descarga el nodo y sus descendientes y encola remove.
func (n *Node) Remove() {
	n.app.mu.Lock()
	defer n.app.mu.Unlock()
	n.app.tree.remove(n.id)
}
