// react-reconciler host config mapping React's mutations onto the gpui-bridge client API.

import { createContext } from "react";
import createReconciler from "react-reconciler";
import { DefaultEventPriority, NoEventPriority } from "react-reconciler/constants.js";
import type {
  App,
  AppWindow,
  NodeType,
  TrayOptions,
  UINode,
  WindowEventName,
  WindowEventPayload,
  WindowOptions,
} from "gpui-bridge";

export interface Container {
  app: App;
  /** The window root; React's top-level children are appended here. */
  root: UINode<"div">;
}

type Handler = (...args: any[]) => void;
type Wire = Record<string, unknown>;
/** Host node types plus `Window` and `Tray`, which add no node to their parent. */
type ElementType = NodeType | "Window" | "Tray";

export interface Instance {
  /** For a `Window`, the root div of its window. */
  node: UINode<NodeType>;
  type: ElementType;
  /** Latest React handlers by protocol event name; host events call through these. */
  handlers: Map<string, Handler>;
  window?: WindowState;
  tray?: TrayState;
}

interface TrayState {
  app: App;
  onClick?: () => void;
  onMenu?: (item: string) => void;
  listener: (event: { event: "click" } | { event: "menu"; item: string }) => void;
  /** React detaches a deleted instance more than once. */
  removed?: boolean;
}

const TRAY_OPTIONS = ["icon", "tooltip", "menu"] as const;

function makeTray(container: Container, props: Record<string, unknown>): Instance {
  const { app } = container;
  const state: TrayState = { app, listener: (e) => (e.event === "menu" ? state.onMenu?.(e.item) : state.onClick?.()) };
  updateTrayHandlers(state, props);
  app.on("tray", state.listener);
  app.tray(trayOptions(TRAY_OPTIONS, props));
  // No node of its own: the container root stands in and is never changed through it.
  return { node: container.root, type: "Tray", handlers: new Map(), tray: state };
}

function trayOptions(keys: readonly (keyof TrayOptions)[], props: Record<string, unknown>): TrayOptions {
  // shortcut: a prop that becomes undefined keeps its last value (the protocol cannot clear tray fields).
  return Object.fromEntries(keys.filter((key) => props[key] !== undefined).map((key) => [key, props[key]]));
}

function updateTrayHandlers(state: TrayState, props: Record<string, unknown>): void {
  state.onClick = props.onClick as TrayState["onClick"];
  state.onMenu = props.onMenu as TrayState["onMenu"];
}

function updateTray(instance: Instance, prevProps: Record<string, unknown>, nextProps: Record<string, unknown>): void {
  const state = instance.tray!;
  updateTrayHandlers(state, nextProps);
  // By value: a `menu` written inline is a new array on every render.
  const changed = TRAY_OPTIONS.filter((key) => JSON.stringify(prevProps[key]) !== JSON.stringify(nextProps[key]));
  const options = trayOptions(changed, nextProps);
  if (Object.keys(options).length) state.app.tray(options);
}

/** `Window` and `Tray` are not nodes of their parent. */
const detached = (instance: Instance) => Boolean(instance.window || instance.tray);

interface WindowState {
  app: App;
  handle: AppWindow;
  onClose?: () => void;
  /** Latest React handler per window event name. */
  handlers: Map<WindowEventName, Handler>;
  /** What each subscribed event calls, so it can be unsubscribed. */
  listeners: Map<WindowEventName, (payload: WindowEventPayload) => void>;
  /** Closed by the user: the host already forgot it. */
  closed: boolean;
}

const WINDOW_OPTIONS = ["title", "width", "height", "x", "y", "minWidth", "minHeight", "minimized", "frameless"] as const;

/** React handler prop per window event (PROTOCOL.md section 3). */
const WINDOW_EVENT_PROPS: Record<string, WindowEventName> = {
  onFocus: "focus",
  onBlur: "blur",
  onResize: "resize",
  onMove: "move",
  onVisibility: "visibility",
};

function windowOptions(props: Record<string, unknown>): WindowOptions {
  return Object.fromEntries(WINDOW_OPTIONS.filter((key) => props[key] !== undefined).map((key) => [key, props[key]]));
}

/** Subscribes or unsubscribes according to the handlers present in `props`. */
function syncWindowHandlers(state: WindowState, props: Record<string, unknown>): void {
  for (const [prop, event] of Object.entries(WINDOW_EVENT_PROPS)) {
    const handler = props[prop];
    if (typeof handler === "function") {
      state.handlers.set(event, handler as Handler);
      if (!state.listeners.has(event)) {
        const listener = (payload: WindowEventPayload) => state.handlers.get(event)?.(payload);
        state.listeners.set(event, listener);
        state.handle.on(event, listener);
      }
    } else {
      state.handlers.delete(event);
      const listener = state.listeners.get(event);
      if (listener) {
        state.handle.off(event, listener);
        state.listeners.delete(event);
      }
    }
  }
}

function makeWindow(app: App, props: Record<string, unknown>): Instance {
  const root = app.div({ class: (props.class as string | undefined) ?? "size-full" });
  const state: WindowState = {
    app,
    handle: app.createWindow({ ...windowOptions(props), root }),
    handlers: new Map(),
    listeners: new Map(),
    closed: false,
  };
  state.onClose = props.onClose as (() => void) | undefined;
  state.handle.on("closed", () => {
    state.closed = true;
    state.onClose?.();
  });
  syncWindowHandlers(state, props);
  return { node: root, type: "Window", handlers: new Map(), window: state };
}

function updateWindow(instance: Instance, prevProps: Record<string, unknown>, nextProps: Record<string, unknown>): void {
  const state = instance.window!;
  state.onClose = nextProps.onClose as (() => void) | undefined;
  const changes = Object.fromEntries(
    WINDOW_OPTIONS.filter((key) => !Object.is(prevProps[key], nextProps[key])).map((key) => [key, nextProps[key]]),
  );
  if (Object.keys(changes).length) state.handle.set(changes);
  syncWindowHandlers(state, nextProps);
  if (prevProps.class !== nextProps.class) (instance.node as UINode<"div">).set({ class: (nextProps.class as string | undefined) ?? "size-full" });
}

/** A `Window` child is not a node of its parent: it has its own window. */
function append(parent: UINode<NodeType>, child: Instance): void {
  if (!detached(child)) parent.append(child.node);
}

function insert(parent: UINode<NodeType>, child: Instance, before: Instance): void {
  if (detached(child)) return;
  // shortcut: before a `Window` sibling (which has no node here) the child goes last; track siblings if order matters.
  if (detached(before)) parent.append(child.node);
  else parent.insertBefore(child.node, before.node);
}

const EVENT_PROPS: Record<string, string> = {
  onClick: "click",
  onDoubleClick: "doubleClick",
  onChange: "change",
  onSubmit: "submit",
  onMenu: "menu",
  onKeyDown: "keyDown",
  onDrop: "drop",
  onOk: "ok",
  onCancel: "cancel",
  onClose: "close",
  onSort: "sort",
  onExpand: "expand",
  onCollapse: "collapse",
  onPointerDown: "pointerDown",
  onConfirm: "confirm",
  onCopied: "copied",
};

function isTextChildren(children: unknown): boolean {
  const items = Array.isArray(children) ? children : [children];
  return items.every((item) => typeof item === "string" || typeof item === "number");
}

function textOf(children: unknown): string {
  return (Array.isArray(children) ? children : [children]).join("");
}

/** Splits React props into wire props and event handlers. */
function split(type: ElementType, props: Record<string, unknown>): { wire: Wire; handlers: Map<string, Handler> } {
  const wire: Wire = {};
  const handlers = new Map<string, Handler>();
  for (const [key, value] of Object.entries(props)) {
    if (key === "children") continue;
    if (key in EVENT_PROPS) {
      if (typeof value === "function") handlers.set(EVENT_PROPS[key], value as Handler);
    } else if (value !== undefined) {
      wire[key] = value;
    }
  }
  if (props.children !== undefined && isTextChildren(props.children)) {
    if (type === "text") wire.content = textOf(props.children);
    if (type === "Button" && wire.label === undefined) wire.label = textOf(props.children);
  }
  // Like React DOM: providing the value makes the component controlled.
  if (type === "Input" || type === "RadioGroup" || type === "Select" || type === "Slider" || type === "Tabs") wire.controlled = props.value !== undefined;
  if (type === "Rating" || type === "Stepper" || type === "Calendar") wire.controlled = props.value !== undefined;
  if (type === "Pagination") wire.controlled = props.page !== undefined;
  if (type === "NumberInput" || type === "OtpInput" || type === "InputGroup") wire.controlled = props.value !== undefined;
  if (type === "Table") wire.controlled = props.selected !== undefined;
  if (type === "Tree" || type === "Carousel" || type === "Sidebar") wire.controlled = props.value !== undefined;
  if (type === "Checkbox" || type === "Switch" || type === "Toggle") wire.controlled = props.checked !== undefined;
  if (type === "DatePicker" || type === "TimeField" || type === "ColorPicker" || type === "Combobox") wire.controlled = props.value !== undefined;
  return { wire, handlers };
}

/** Host event payloads become plain handler arguments. */
function subscribe(instance: Instance, event: string): void {
  const node = instance.node as UINode<any>;
  node.on(event, (payload: { value?: string | number | null | string[]; checked?: boolean; item?: string; key?: string; paths?: string[] }) => {
    const handler = instance.handlers.get(event);
    if (!handler) return;
    if (event === "click" || event === "ok" || event === "cancel" || event === "close") handler();
    // A Table's double click carries the row.
    else if (event === "doubleClick") instance.type === "Table" ? handler(payload.value) : handler();
    else if (event === "sort") handler(payload);
    else if (event === "menu") handler(payload.item);
    else if (event === "keyDown") handler(payload.key);
    else if (event === "drop") handler(payload.paths);
    else if (instance.type === "Checkbox" || instance.type === "Switch" || instance.type === "Toggle") handler(payload.checked);
    else if (instance.type === "Popover" || instance.type === "HoverCard") handler(payload.checked);
    else handler(payload.value);
  });
}

function createNode(app: App, type: NodeType, wire: Wire): UINode<NodeType> {
  switch (type) {
    case "div":
      return app.div(wire);
    case "text":
      return app.text(wire);
    case "Button":
      return app.button(wire);
    case "Input":
      return app.input(wire);
    case "Checkbox":
      return app.checkbox(wire);
    case "Icon":
      return app.icon(wire);
    case "Image":
      return app.image(wire);
    case "MenuBar":
      return app.menuBar(wire);
    case "Switch":
      return app.switch(wire);
    case "RadioGroup":
      return app.radioGroup(wire);
    case "Select":
      return app.select(wire);
    case "Slider":
      return app.slider(wire);
    case "Textarea":
      return app.textarea(wire);
    case "Dialog":
      return app.dialog(wire);
    case "VirtualList":
      return app.virtualList(wire);
    case "Table":
      return app.table(wire);
    case "Tabs":
      return app.tabs(wire);
    case "Tab":
      return app.tab(wire);
    case "Alert":
      return app.alert(wire);
    case "Badge":
      return app.badge(wire);
    case "Tag":
      return app.tag(wire);
    case "Kbd":
      return app.kbd(wire);
    case "Progress":
      return app.progress(wire);
    case "Spinner":
      return app.spinner(wire);
    case "Skeleton":
      return app.skeleton(wire);
    case "Rating":
      return app.rating(wire);
    case "Pagination":
      return app.pagination(wire);
    case "Collapsible":
      return app.collapsible(wire);
    case "Accordion":
      return app.accordion(wire);
    case "Avatar":
      return app.avatar(wire);
    case "Stepper":
      return app.stepper(wire);
    case "NumberInput":
      return app.numberInput(wire);
    case "OtpInput":
      return app.otpInput(wire);
    case "Form":
      return app.form(wire);
    case "Field":
      return app.field(wire);
    case "GroupBox":
      return app.groupBox(wire);
    case "DescriptionList":
      return app.descriptionList(wire);
    case "StatusBar":
      return app.statusBar(wire);
    case "Toolbar":
      return app.toolbar(wire);
    case "Resizable":
      return app.resizable(wire);
    case "Calendar":
      return app.calendar(wire);
    case "Tree":
      return app.tree(wire);
    case "Diff":
      return app.diff(wire);
    case "Carousel":
      return app.carousel(wire);
    case "Sidebar":
      return app.sidebar(wire);
    case "Chart":
      return app.chart(wire);
    case "Canvas":
      return app.canvas(wire);
    case "Command":
      return app.command(wire);
    case "Settings":
      return app.settings(wire);
    case "Toggle":
      return app.toggle(wire);
    case "DropdownButton":
      return app.dropdownButton(wire);
    case "Label":
      return app.label(wire);
    case "Empty":
      return app.empty(wire);
    case "Shimmer":
      return app.shimmer(wire);
    case "InputGroup":
      return app.inputGroup(wire);
    case "Clipboard":
      return app.clipboard(wire);
    case "AlertDialog":
      return app.alertDialog(wire);
    case "Sheet":
      return app.sheet(wire);
    case "Popover":
      return app.popover(wire);
    case "HoverCard":
      return app.hoverCard(wire);
    case "DatePicker":
      return app.datePicker(wire);
    case "TimeField":
      return app.timeField(wire);
    case "ColorPicker":
      return app.colorPicker(wire);
    case "Combobox":
      return app.combobox(wire);
    default:
      throw new Error(`gpui-bridge-react: unknown element type '${type}'`);
  }
}

function makeInstance(container: Container, type: ElementType, props: Record<string, unknown>): Instance {
  if (type === "Window") return makeWindow(container.app, props);
  if (type === "Tray") return makeTray(container, props);
  const { wire, handlers } = split(type, props);
  const instance: Instance = { node: createNode(container.app, type, wire), type, handlers };
  for (const event of handlers.keys()) subscribe(instance, event);
  return instance;
}

let currentUpdatePriority: number = NoEventPriority;

export const reconciler = createReconciler<
  ElementType, // Type
  Record<string, unknown>, // Props
  Container,
  Instance,
  Instance, // TextInstance
  never, // ActivityInstance
  never, // SuspenseInstance
  never, // HydratableInstance
  never, // FormInstance
  UINode<NodeType>, // PublicInstance
  {}, // HostContext
  never, // ChildSet
  ReturnType<typeof setTimeout>,
  -1, // NoTimeout
  null, // TransitionStatus
  null, // SuspendedState
  never, // RendererInspectionConfig
  never, // FormStateMarkerInstance
  never, // HoistableRoot
  never // Resource
>({
  rendererPackageName: "gpui-bridge-react",
  rendererVersion: "0.1.0",
  extraDevToolsConfig: null,
  supportsMutation: true,
  supportsPersistence: false,
  supportsHydration: false,
  isPrimaryRenderer: true,

  createInstance: (type, props, container) => makeInstance(container, type, props),
  createTextInstance: (text, container) => makeInstance(container, "text", { children: text }),
  shouldSetTextContent: (type, props) =>
    (type === "text" || type === "Button") && props.children !== undefined && isTextChildren(props.children),
  finalizeInitialChildren: () => false,

  appendInitialChild: (parent, child) => append(parent.node, child),
  appendChild: (parent, child) => append(parent.node, child),
  appendChildToContainer: (container, child) => append(container.root, child),
  insertBefore: (parent, child, before) => insert(parent.node, child, before),
  insertInContainerBefore: (container, child, before) => insert(container.root, child, before),
  // A removed `Window` (or one inside a removed subtree) is closed in `detachDeletedInstance`.
  removeChild: (_parent, child) => void (detached(child) || child.node.remove()),
  removeChildFromContainer: (_container, child) => void (detached(child) || child.node.remove()),
  // The root starts empty and is only populated by React.
  clearContainer: () => {},

  commitUpdate(instance, type, prevProps, nextProps) {
    if (instance.window) return updateWindow(instance, prevProps, nextProps);
    if (instance.tray) return updateTray(instance, prevProps, nextProps);
    const prev = split(type, prevProps);
    const next = split(type, nextProps);
    const changes: Wire = {};
    for (const key of new Set([...Object.keys(prev.wire), ...Object.keys(next.wire)])) {
      if (!(key in next.wire)) changes[key] = null;
      else if (!Object.is(prev.wire[key], next.wire[key])) changes[key] = next.wire[key];
    }
    if (Object.keys(changes).length) (instance.node as UINode<any>).set(changes);

    for (const event of prev.handlers.keys()) {
      if (!next.handlers.has(event)) (instance.node as UINode<any>).off(event);
    }
    for (const event of next.handlers.keys()) {
      if (!prev.handlers.has(event)) subscribe(instance, event);
    }
    instance.handlers = next.handlers;
  },
  commitTextUpdate: (instance, _old, text) => void (instance.node as UINode<"text">).set({ content: text }),
  resetTextContent: (instance) => void (instance.node as UINode<any>).set({ content: null, label: null }),

  // One host render per React commit.
  resetAfterCommit: (container) => container.app.commit(),
  prepareForCommit: () => null,
  getRootHostContext: () => ({}),
  getChildHostContext: (parent) => parent,
  getPublicInstance: (instance) => instance.node,
  preparePortalMount: () => {},

  scheduleTimeout: setTimeout,
  cancelTimeout: clearTimeout,
  noTimeout: -1,
  supportsMicrotasks: true,
  scheduleMicrotask: queueMicrotask,

  setCurrentUpdatePriority: (priority) => void (currentUpdatePriority = priority),
  getCurrentUpdatePriority: () => currentUpdatePriority,
  resolveUpdatePriority: () => currentUpdatePriority || DefaultEventPriority,

  // Features this renderer does not use (transitions, forms, suspense commits, scopes).
  NotPendingTransition: null,
  HostTransitionContext: createContext<null>(null) as any,
  resetFormInstance: () => {},
  requestPostPaintCallback: () => {},
  shouldAttemptEagerTransition: () => false,
  trackSchedulerEvent: () => {},
  resolveEventType: () => null,
  resolveEventTimeStamp: () => -1.1,
  maySuspendCommit: () => false,
  maySuspendCommitOnUpdate: () => false,
  maySuspendCommitInSyncRender: () => false,
  preloadInstance: () => true,
  startSuspendingCommit: () => null,
  suspendInstance: () => {},
  suspendOnActiveViewTransition: () => {},
  waitForCommitToBeReady: () => null,
  getSuspendedCommitReason: () => null,
  getInstanceFromNode: () => null,
  beforeActiveInstanceBlur: () => {},
  afterActiveInstanceBlur: () => {},
  prepareScopeUpdate: () => {},
  getInstanceFromScope: () => null,
  // Called for every deleted host instance, after the commit: a deleted `Window` or `Tray` goes away with a commit of its own.
  detachDeletedInstance(instance) {
    if (instance.tray) {
      if (instance.tray.removed) return;
      instance.tray.removed = true;
      instance.tray.app.off("tray", instance.tray.listener);
      instance.tray.app.removeTray().commit();
      return;
    }
    const state = instance.window;
    if (!state || state.closed) return;
    state.handle.close();
    instance.node.remove();
    state.closed = true;
    state.app.commit();
  },
  bindToConsole: (method, args) => (console as any)[method].bind(console, ...args),
});
