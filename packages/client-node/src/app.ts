// Runtime-agnostic client core: the App/UINode API and the handshake. It only
// needs a transport to the host process, so any JavaScript runtime can use it
// (Node: index.ts, txiki.js: txiki.ts).

import {
  PROTOCOL_VERSION,
  type ClientMessage,
  type ErrorCode,
  type HostMessage,
  type MenuItem,
  type NodeEvents,
  type NodeId,
  type NodeProps,
  type NodeType,
  type TableSort,
  type ThemeSet,
  type WindowId,
  type WindowTheme,
} from "./protocol.js";
import { Tree } from "./tree.js";

const HANDSHAKE_TIMEOUT_MS = 10_000;

export interface HostExit {
  code: number | null;
  signal: string | null;
  /** Last lines the host wrote to stderr, for crash reports. */
  stderrTail: string[];
  /** Set when the process could not be started at all (e.g. binary not found). */
  spawnError?: Error;
}

/** A running host process, as seen by the client core. */
export interface HostTransport {
  send(message: ClientMessage): void;
  /** Closes stdin; the host exits on EOF. */
  close(): void;
  kill(): void;
}

/** What a transport reports back while the host runs. */
export interface HostEvents {
  onMessage(message: HostMessage): void;
  /** stdout carried something that is not a protocol message. */
  onCorruptLine(line: string): void;
  onExit(exit: HostExit): void;
}

export interface WindowOptions {
  title?: string;
  width?: number;
  height?: number;
  root?: UINode<NodeType>;
  theme?: WindowTheme;
  themeSet?: ThemeSet;
  /** No system title bar (draw your own with `windowControl` divs). Only read when the window opens. */
  frameless?: boolean;
}

/** A toast (PROTOCOL.md section 14). */
export interface NotifyOptions {
  message: string;
  title?: string;
  type?: "info" | "success" | "warning" | "error";
  /** Default window 1. */
  window?: WindowId;
  /** Lets a click be reported (`notification` event); a new toast with the same id replaces it. */
  id?: string;
  /** Default true. */
  autohide?: boolean;
  /** Also post it to the Windows notification center. */
  system?: boolean;
}

/** `openDialog` options (PROTOCOL.md section 13). Default: one file. */
export interface OpenDialogOptions {
  files?: boolean;
  directories?: boolean;
  multiple?: boolean;
  /** The confirm button's text. */
  prompt?: string;
}

/** `saveDialog` options. */
export interface SaveDialogOptions {
  /** Where the dialog starts; default the host's working directory. */
  directory?: string;
  /** Suggested file name. */
  name?: string;
}

/** The system tray icon (PROTOCOL.md section 11). Each call updates only the fields it passes. */
export interface TrayOptions {
  /** An image file (PNG, ICO…), drawn at the tray's size. */
  icon?: string;
  tooltip?: string;
  /** Opens on right-click; a native menu, so item icons are not drawn. */
  menu?: MenuItem[];
}

/** An `error` message from the host: the operation was rejected, the host keeps running. */
export interface HostError {
  code: ErrorCode;
  message: string;
  op?: string;
}

interface AppEvents {
  error: HostError;
  /** The user closed a window. */
  windowClosed: { id: WindowId };
  /** The user clicked a toast that has an `id`. */
  notification: { id: string; window: WindowId };
  /** A key registered with `shortcuts()` was pressed in window `window`. */
  shortcut: { keys: string; window: WindowId };
  /** The user clicked the tray icon (`click`) or chose an item of its menu (`menu`, with the item id). */
  tray: { event: "click" } | { event: "menu"; item: string };
  exit: HostExit;
}

/** `setProps` input: `null` removes a prop. */
export type PropsUpdate<T extends NodeType> = { [K in keyof NodeProps[T]]?: NodeProps[T][K] | null };

export class HostExitedError extends Error {
  constructor(
    message: string,
    readonly exit: HostExit,
  ) {
    super(message);
    this.name = "HostExitedError";
  }
}

function describeExit(exit: HostExit, hostPath: string): string {
  if (exit.spawnError) {
    const code = (exit.spawnError as { code?: string }).code;
    return code === "ENOENT"
      ? `gpui-bridge host not found at ${hostPath}. Build it with \`cargo build\` in host/ or pass hostPath.`
      : `gpui-bridge host failed to start: ${exit.spawnError.message}`;
  }
  const how = exit.signal ? `signal ${exit.signal}` : `code ${exit.code}`;
  const tail = exit.stderrTail.length ? `\nLast host output:\n  ${exit.stderrTail.join("\n  ")}` : "";
  return `gpui-bridge host exited (${how}).${tail}`;
}

export class UINode<T extends NodeType> {
  /** @internal */
  constructor(
    private readonly app: App,
    readonly id: NodeId,
    readonly type: T,
  ) {}

  append(...children: UINode<NodeType>[]): this {
    for (const child of children) {
      this.app.tree.queue({ op: "append", parent: this.id, child: child.id });
      this.app.tree.attach(this.id, child.id);
    }
    return this;
  }

  insertBefore(child: UINode<NodeType>, before: UINode<NodeType>): this {
    this.app.tree.queue({ op: "insertBefore", parent: this.id, child: child.id, before: before.id });
    this.app.tree.attach(this.id, child.id);
    return this;
  }

  /** Merges props; `null` removes one. */
  set(props: PropsUpdate<T>): this {
    this.app.tree.queue({ op: "setProps", id: this.id, props });
    return this;
  }

  /** Subscribes to an event; the host only emits subscribed events. */
  on<E extends keyof NodeEvents[T] & string>(event: E, handler: (payload: NodeEvents[T][E]) => void): this {
    this.app.tree.setHandler(this.id, event, handler);
    return this;
  }

  off<E extends keyof NodeEvents[T] & string>(event: E): this {
    this.app.tree.setHandler(this.id, event, undefined);
    return this;
  }

  /** Gives the node the keyboard focus on the next commit (an Input, a Textarea or a div with `keyDown`). */
  focus(): this {
    this.app.tree.queue({ op: "focus", id: this.id });
    return this;
  }

  /** Removes the node and all its descendants. */
  remove(): void {
    this.app.tree.remove(this.id);
  }
}

/** A window other than the main one (`App.createWindow`). */
export class AppWindow {
  private readonly closedListeners = new Set<() => void>();

  /** @internal */
  constructor(
    private readonly app: App,
    readonly id: WindowId,
  ) {}

  /** Updates the window; like `App.window`, it opens on the first commit that sets `root`. */
  set(options: WindowOptions): this {
    this.app.queueWindow(this.id, options);
    return this;
  }

  /** Closes the window on the next commit. No `closed` event follows: that is only for the user closing it. */
  close(): void {
    this.app.queueCloseWindow(this.id);
  }

  /** The user closed the window. */
  on(event: "closed", listener: () => void): this {
    this.closedListeners.add(listener);
    return this;
  }

  off(event: "closed", listener: () => void): this {
    this.closedListeners.delete(listener);
    return this;
  }

  /** @internal */
  handleClosed(): void {
    for (const listener of this.closedListeners) listener();
  }
}

export class App {
  /** @internal */
  readonly tree = new Tree();
  private readonly listeners: { [K in keyof AppEvents]: Set<(payload: AppEvents[K]) => void> } = {
    error: new Set(),
    windowClosed: new Set(),
    shortcut: new Set(),
    notification: new Set(),
    tray: new Set(),
    exit: new Set(),
  };
  private exitInfo?: HostExit;
  private closing = false;
  private nextWindowId = 2;
  private readonly windows = new Map<WindowId, AppWindow>();
  /** Windows given a root and not closed: the host exits when the user closes the last one. */
  private readonly openWindows = new Set<WindowId>();
  private nextRequest = 1;
  /** System actions waiting for their `result`. */
  private readonly requests = new Map<number, { resolve: (result: Extract<HostMessage, { type: "result" }>) => void; reject: (err: Error) => void }>();
  /** A tray keeps the host running after the last window closes. */
  private hasTray = false;
  private readonly exitWaiters: ((exit: HostExit) => void)[] = [];

  /** @internal */
  constructor(
    private readonly host: HostTransport,
    private readonly hostPath: string,
  ) {}

  div(props: NodeProps["div"] = {}) {
    return this.create("div", props);
  }

  text(props: NodeProps["text"] = {}) {
    return this.create("text", props);
  }

  button(props: NodeProps["Button"] = {}) {
    return this.create("Button", props);
  }

  input(props: NodeProps["Input"] = {}) {
    return this.create("Input", props);
  }

  checkbox(props: NodeProps["Checkbox"] = {}) {
    return this.create("Checkbox", props);
  }

  icon(props: NodeProps["Icon"] = {}) {
    return this.create("Icon", props);
  }

  image(props: NodeProps["Image"] = {}) {
    return this.create("Image", props);
  }

  switch(props: NodeProps["Switch"] = {}) {
    return this.create("Switch", props);
  }

  radioGroup(props: NodeProps["RadioGroup"] = {}) {
    return this.create("RadioGroup", props);
  }

  select(props: NodeProps["Select"] = {}) {
    return this.create("Select", props);
  }

  slider(props: NodeProps["Slider"] = {}) {
    return this.create("Slider", props);
  }

  textarea(props: NodeProps["Textarea"] = {}) {
    return this.create("Textarea", props);
  }

  /** A modal dialog shown while `open`; append its body as children. */
  dialog(props: NodeProps["Dialog"] = {}) {
    return this.create("Dialog", props);
  }

  /** A list that builds only the rows in view; append one child per row (PROTOCOL.md section 15). */
  virtualList(props: NodeProps["VirtualList"] = {}) {
    return this.create("VirtualList", props);
  }

  /** A data table with text cells; it virtualizes its rows. */
  table(props: NodeProps["Table"] = {}) {
    return this.create("Table", props);
  }

  /** A tab bar; show the chosen tab's content yourself. */
  tabs(props: NodeProps["Tabs"] = {}) {
    return this.create("Tabs", props);
  }

  /** A menu bar: one button per menu (PROTOCOL.md section 10). */
  menuBar(props: NodeProps["MenuBar"] = {}) {
    return this.create("MenuBar", props);
  }

  /** An inline message; `close` when it is `closable` (PROTOCOL.md section 17). */
  alert(props: NodeProps["Alert"] = {}) {
    return this.create("Alert", props);
  }

  /** A count, a dot or an icon over its children. */
  badge(props: NodeProps["Badge"] = {}) {
    return this.create("Badge", props);
  }

  tag(props: NodeProps["Tag"] = {}) {
    return this.create("Tag", props);
  }

  /** A key or shortcut, such as `ctrl-s`. */
  kbd(props: NodeProps["Kbd"] = {}) {
    return this.create("Kbd", props);
  }

  /** A progress bar: `value` from 0 to 100. */
  progress(props: NodeProps["Progress"] = {}) {
    return this.create("Progress", props);
  }

  /** A spinner; give it a size with `class` (`size-4`). */
  spinner(props: NodeProps["Spinner"] = {}) {
    return this.create("Spinner", props);
  }

  /** A loading placeholder. */
  skeleton(props: NodeProps["Skeleton"] = {}) {
    return this.create("Skeleton", props);
  }

  /** Stars: `change` with the chosen count. */
  rating(props: NodeProps["Rating"] = {}) {
    return this.create("Rating", props);
  }

  /** Page navigation: `change` with the chosen page (1-based). */
  pagination(props: NodeProps["Pagination"] = {}) {
    return this.create("Pagination", props);
  }

  /** Content shown while `open`; toggle it with anything that takes a click. */
  collapsible(props: NodeProps["Collapsible"] = {}) {
    return this.create("Collapsible", props);
  }

  /** Sections as `items`; `change` with the id of the one toggled. */
  accordion(props: NodeProps["Accordion"] = {}) {
    return this.create("Accordion", props);
  }

  /** A user picture, or their initial. */
  avatar(props: NodeProps["Avatar"] = {}) {
    return this.create("Avatar", props);
  }

  /** A step indicator: `change` with the chosen step's `value`. */
  stepper(props: NodeProps["Stepper"] = {}) {
    return this.create("Stepper", props);
  }

  /** A text field with increment and decrement buttons. */
  numberInput(props: NodeProps["NumberInput"] = {}) {
    return this.create("NumberInput", props);
  }

  /** A one-time code, one cell per digit. */
  otpInput(props: NodeProps["OtpInput"] = {}) {
    return this.create("OtpInput", props);
  }

  /** A form; append `Field`s to it. */
  form(props: NodeProps["Form"] = {}) {
    return this.create("Form", props);
  }

  /** A label, an optional description and its children as the control. */
  field(props: NodeProps["Field"] = {}) {
    return this.create("Field", props);
  }

  /** A titled box; append its content. */
  groupBox(props: NodeProps["GroupBox"] = {}) {
    return this.create("GroupBox", props);
  }

  /** Key-value rows. */
  descriptionList(props: NodeProps["DescriptionList"] = {}) {
    return this.create("DescriptionList", props);
  }

  /** A status bar with text on the left and on the right. */
  statusBar(props: NodeProps["StatusBar"] = {}) {
    return this.create("StatusBar", props);
  }

  /** A toolbar; append its content. */
  toolbar(props: NodeProps["Toolbar"] = {}) {
    return this.create("Toolbar", props);
  }

  /** Panels the user can resize; append one child per panel. */
  resizable(props: NodeProps["Resizable"] = {}) {
    return this.create("Resizable", props);
  }

  /** A month view: `change` with the chosen day (`YYYY-MM-DD`). */
  calendar(props: NodeProps["Calendar"] = {}) {
    return this.create("Calendar", props);
  }

  /** A node of any type; the specific methods below only add typing. */
  create<T extends NodeType>(type: T, props: NodeProps[T] = {} as NodeProps[T]): UINode<T> {
    return new UINode(this, this.tree.create(type, { ...props }), type);
  }

  /** Creates or updates the main window (window 1). It opens on the first commit that sets `root`. */
  window(options: WindowOptions): this {
    this.queueWindow(undefined, options);
    return this;
  }

  /** Another window, with its own root; it opens on the first commit that sets `root`. */
  createWindow(options: WindowOptions = {}): AppWindow {
    const window = new AppWindow(this, this.nextWindowId++);
    this.windows.set(window.id, window);
    return window.set(options);
  }

  /** @internal */
  queueWindow(id: WindowId | undefined, options: WindowOptions): void {
    const { root, ...rest } = options;
    this.tree.queue({ op: "window", ...(id !== undefined && { id }), ...rest, ...(root && { root: root.id }) });
    if (root) this.openWindows.add(id ?? 1);
  }

  /** @internal */
  queueCloseWindow(id: WindowId): void {
    this.tree.queue({ op: "closeWindow", id });
    this.openWindows.delete(id);
  }

  /** Native "open" dialog; resolves with the chosen paths, or `null` if cancelled. */
  async openDialog(options: OpenDialogOptions = {}): Promise<string[] | null> {
    return (await this.request({ op: "openDialog", request: 0, ...options })).paths ?? null;
  }

  /** Native "save" dialog; resolves with the chosen path, or `null` if cancelled. */
  async saveDialog(options: SaveDialogOptions = {}): Promise<string | null> {
    return (await this.request({ op: "saveDialog", request: 0, ...options })).path ?? null;
  }

  /** The clipboard's text, or `null` if it holds none. */
  async readClipboard(): Promise<string | null> {
    return (await this.request({ op: "readClipboard", request: 0 })).text ?? null;
  }

  /** Shows a toast now (not on commit). */
  notify(options: NotifyOptions): void {
    this.action({ op: "notify", ...options });
  }

  writeClipboard(text: string): void {
    this.action({ op: "writeClipboard", text });
  }

  /** Opens a URL with the system's handler (the browser for http). */
  openUrl(url: string): void {
    this.action({ op: "openUrl", url });
  }

  /** Opens a file or folder with its default app. */
  openPath(path: string): void {
    this.action({ op: "openPath", path });
  }

  /** Actions run when the host gets them, not on commit. */
  private action(message: ClientMessage): void {
    if (this.exitInfo) throw new HostExitedError(describeExit(this.exitInfo, this.hostPath), this.exitInfo);
    this.host.send(message);
  }

  private request(message: Extract<ClientMessage, { request: number }>): Promise<Extract<HostMessage, { type: "result" }>> {
    const request = this.nextRequest++;
    return new Promise((resolve, reject) => {
      this.action({ ...message, request });
      this.requests.set(request, { resolve, reject });
    });
  }

  /** App-wide keyboard shortcuts (`ctrl-s`, `f5`…), applied on the next commit; replaces the list. */
  shortcuts(keys: string[]): this {
    this.tree.queue({ op: "shortcuts", keys });
    return this;
  }

  /** Shows or updates the system tray icon on the next commit. */
  tray(options: TrayOptions): this {
    this.tree.queue({ op: "tray", ...options });
    this.hasTray = true;
    return this;
  }

  /** Removes the tray icon on the next commit; with no window open, the host then exits. */
  removeTray(): this {
    this.tree.queue({ op: "removeTray" });
    this.hasTray = false;
    if (this.openWindows.size === 0) this.closing = true;
    return this;
  }

  /** Sends every pending operation; the host renders once per commit. */
  commit(): void {
    if (this.exitInfo) throw new HostExitedError(describeExit(this.exitInfo, this.hostPath), this.exitInfo);
    this.tree.commit((message) => this.host.send(message));
  }

  on<K extends keyof AppEvents>(event: K, listener: (payload: AppEvents[K]) => void): this {
    this.listeners[event].add(listener);
    return this;
  }

  off<K extends keyof AppEvents>(event: K, listener: (payload: AppEvents[K]) => void): this {
    this.listeners[event].delete(listener);
    return this;
  }

  /** Closes the connection; resolves when the host has exited. */
  close(): Promise<HostExit> {
    this.closing = true;
    this.host.close();
    return this.exited();
  }

  /** Resolves when the host process has exited, for any reason. */
  exited(): Promise<HostExit> {
    if (this.exitInfo) return Promise.resolve(this.exitInfo);
    return new Promise((resolve) => this.exitWaiters.push(resolve));
  }

  /** @internal */
  handleMessage(message: HostMessage): void {
    switch (message.type) {
      case "event": {
        const handler = this.tree.handler(message.id, message.event);
        if (!handler) return;
        const type = this.tree.type(message.id);
        const payload =
          message.event === "menu"
            ? { item: message.value ?? "" }
            : message.event === "keyDown"
              ? { key: message.value ?? "" }
              : message.event === "drop"
                ? { paths: message.paths ?? [] }
              : message.event === "sort"
                ? (message.value as TableSort)
            : type === "Checkbox" || type === "Switch"
              ? { checked: message.checked ?? false }
              : message.value !== undefined
                ? { value: message.value }
                : type === "Input" || type === "Textarea"
                  ? { value: "" }
                  : {};
        handler(payload);
        return;
      }
      case "error": {
        const error: HostError = { code: message.code, message: message.message, op: message.op };
        // A rejected system action: its promise fails instead of waiting for a result.
        const pending = message.request !== undefined ? this.requests.get(message.request) : undefined;
        if (pending) {
          this.requests.delete(message.request!);
          pending.reject(Object.assign(new Error(`gpui-bridge: host rejected '${error.op}': ${error.message}`), { host: error }));
          return;
        }
        if (!this.emit("error", error)) console.warn(`gpui-bridge: host rejected '${error.op}': ${error.message}`);
        return;
      }
      case "windowClosed":
        this.openWindows.delete(message.id);
        // The host exits after the last window, unless there is a tray.
        if (this.openWindows.size === 0 && !this.hasTray) this.closing = true;
        this.windows.get(message.id)?.handleClosed();
        this.emit("windowClosed", { id: message.id });
        return;
      case "result":
        this.requests.get(message.request)?.resolve(message);
        this.requests.delete(message.request);
        return;
      case "notification":
        this.emit("notification", { id: message.id, window: message.window });
        return;
      case "shortcut":
        this.emit("shortcut", { keys: message.keys, window: message.window });
        return;
      case "tray":
        this.emit("tray", message.event === "menu" ? { event: "menu", item: message.value ?? "" } : { event: "click" });
        return;
      case "ready":
        return;
    }
  }

  /** @internal */
  handleExit(exit: HostExit): void {
    this.exitInfo = exit;
    // An exit after close() or windowClosed is expected; anything else is a crash.
    const unexpected = !this.closing;
    if (!this.emit("exit", exit) && unexpected) console.error(describeExit(exit, this.hostPath));
    for (const resolve of this.exitWaiters.splice(0)) resolve(exit);
    for (const { reject } of this.requests.values()) reject(new HostExitedError(describeExit(exit, this.hostPath), exit));
    this.requests.clear();
  }

  private emit<K extends keyof AppEvents>(event: K, payload: AppEvents[K]): boolean {
    for (const listener of this.listeners[event]) listener(payload);
    return this.listeners[event].size > 0;
  }
}

/**
 * Starts the host through `start` and completes the handshake. Rejects with a
 * clear message if the host cannot start.
 */
export function connect(hostPath: string, start: (events: HostEvents) => HostTransport): Promise<App> {
  return new Promise((resolve, reject) => {
    let app: App | undefined;
    let ready = false;
    const timer = setTimeout(() => {
      host.kill();
      reject(new Error(`gpui-bridge host did not answer the handshake within ${HANDSHAKE_TIMEOUT_MS} ms`));
    }, HANDSHAKE_TIMEOUT_MS);

    const host = start({
      onMessage(message) {
        if (ready) return app!.handleMessage(message);
        if (message.type === "ready") {
          ready = true;
          clearTimeout(timer);
          resolve(app!);
        } else if (message.type === "error") {
          clearTimeout(timer);
          reject(new Error(`gpui-bridge handshake failed (${message.code}): ${message.message}`));
        }
      },
      onCorruptLine(line) {
        console.error(`gpui-bridge: host wrote a non-protocol line to stdout: ${line}`);
      },
      onExit(exit) {
        if (ready) return app!.handleExit(exit);
        clearTimeout(timer);
        reject(new HostExitedError(describeExit(exit, hostPath), exit));
      },
    });
    app = new App(host, hostPath);
    host.send({ op: "hello", protocol: PROTOCOL_VERSION });
  });
}
