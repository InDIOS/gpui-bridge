// Client-side bookkeeping: id allocation, batched operations and event handlers.

import type { ClientMessage, NodeId, NodeType } from "./protocol.js";

export type EventHandler = (payload: any) => void;

interface NodeRecord {
  type: NodeType;
  parent?: NodeId;
  children: Set<NodeId>;
  handlers: Map<string, EventHandler>;
}

export class Tree {
  private nextId: NodeId = 1;
  private pending: ClientMessage[] = [];
  private readonly nodes = new Map<NodeId, NodeRecord>();

  /** Ids are positive, unique for the whole session and never reused. */
  create(type: NodeType, props: Record<string, unknown>): NodeId {
    const id = this.nextId++;
    this.nodes.set(id, { type, children: new Set(), handlers: new Map() });
    this.pending.push({ op: "create", id, type, props });
    return id;
  }

  type(id: NodeId): NodeType | undefined {
    return this.nodes.get(id)?.type;
  }

  queue(op: ClientMessage): void {
    this.pending.push(op);
  }

  /** Mirrors the host's move semantics so `remove` can drop descendant handlers. */
  attach(parent: NodeId, child: NodeId): void {
    const record = this.nodes.get(child);
    if (!record) return;
    if (record.parent !== undefined) this.nodes.get(record.parent)?.children.delete(child);
    record.parent = parent;
    this.nodes.get(parent)?.children.add(child);
  }

  remove(id: NodeId): void {
    const record = this.nodes.get(id);
    if (!record) return;
    if (record.parent !== undefined) this.nodes.get(record.parent)?.children.delete(id);
    const stack = [id];
    while (stack.length) {
      const current = stack.pop()!;
      const node = this.nodes.get(current);
      if (!node) continue;
      stack.push(...node.children);
      this.nodes.delete(current);
    }
    this.pending.push({ op: "remove", id });
  }

  /** Sets or clears a handler; queues `setEvents` when the subscription set changes. */
  setHandler(id: NodeId, event: string, handler: EventHandler | undefined): void {
    const record = this.nodes.get(id);
    if (!record) return;
    const had = record.handlers.has(event);
    if (handler) record.handlers.set(event, handler);
    else record.handlers.delete(event);
    if (had !== record.handlers.has(event)) {
      this.pending.push({ op: "setEvents", id, events: [...record.handlers.keys()] });
    }
  }

  handler(id: NodeId, event: string): EventHandler | undefined {
    return this.nodes.get(id)?.handlers.get(event);
  }

  /** Sends every queued operation, then `commit`. */
  commit(send: (message: ClientMessage) => void): void {
    for (const op of this.pending) send(op);
    this.pending = [];
    send({ op: "commit" });
  }
}
