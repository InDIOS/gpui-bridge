"""gpui-bridge Python client: drives the gpui-bridge host over JSON Lines (stdlib only).

Same model as the Node client: build nodes, queue operations, `commit()` sends
them in one batch. `run()` blocks and calls event handlers on the calling thread
until the host exits.
"""

import json
import os
import queue
import subprocess
import sys
import threading
import time
from collections import deque
from dataclasses import dataclass, field
from pathlib import Path

PROTOCOL_VERSION = 1
HANDSHAKE_TIMEOUT = 10.0
STDERR_TAIL_LINES = 20
# Window events a client can subscribe to (PROTOCOL.md section 3).
WINDOW_EVENTS = ("focus", "blur", "resize", "move", "visibility")


@dataclass
class HostExit:
    code: int | None
    # Last lines the host wrote to stderr, for crash reports.
    stderr_tail: list[str]


@dataclass
class HostError:
    """An `error` message from the host: the operation was rejected, the host keeps running."""

    code: str
    message: str
    op: str | None = None


class HostExitedError(Exception):
    def __init__(self, message: str, exit: HostExit):
        super().__init__(message)
        self.exit = exit


@dataclass
class _Record:
    type: str
    parent: int | None = None
    children: set[int] = field(default_factory=set)
    handlers: dict = field(default_factory=dict)


def default_host_path() -> str:
    """`$GPUI_BRIDGE_HOST`, then the host bundled in this package, then a Cargo build in this repository."""
    if env := os.environ.get("GPUI_BRIDGE_HOST"):
        return env
    exe = "gpui-bridge-host.exe" if sys.platform == "win32" else "gpui-bridge-host"
    package = Path(__file__).resolve().parent
    # build_wheel.py puts the optimized host in bin/; a checkout of this repository has none.
    bundled = package / "bin" / exe
    if bundled.exists():
        return str(bundled)
    # Walk up for the workspace `target/`: how many folders separate this client from the
    # repository root is not part of the contract, so a move must not break the lookup.
    for parent in package.parents:
        for profile in ("debug", "release"):
            candidate = parent / "target" / profile / exe
            if candidate.exists():
                return str(candidate)
    return str(package.parents[2] / "target" / "debug" / exe)


def _props(kwargs: dict) -> dict:
    # `class` is a Python keyword, so callers write `class_`.
    return {key.rstrip("_"): value for key, value in kwargs.items()}


def _describe_exit(exit: HostExit) -> str:
    tail = "\nLast host output:\n  " + "\n  ".join(exit.stderr_tail) if exit.stderr_tail else ""
    return f"gpui-bridge host exited (code {exit.code}).{tail}"


class Node:
    def __init__(self, app: "App", id: int, type: str):
        self._app = app
        self.id = id
        self.type = type

    def append(self, *children: "Node") -> "Node":
        for child in children:
            self._app._pending.append({"op": "append", "parent": self.id, "child": child.id})
            self._app._attach(self.id, child.id)
        return self

    def insert_before(self, child: "Node", before: "Node") -> "Node":
        self._app._pending.append({"op": "insertBefore", "parent": self.id, "child": child.id, "before": before.id})
        self._app._attach(self.id, child.id)
        return self

    def set(self, **props) -> "Node":
        """Merges props; `None` removes one."""
        self._app._pending.append({"op": "setProps", "id": self.id, "props": _props(props)})
        return self

    def on(self, event: str, handler) -> "Node":
        """Subscribes to an event. Handlers get the Input text, the Checkbox state, or nothing (click)."""
        self._app._set_handler(self.id, event, handler)
        return self

    def off(self, event: str) -> "Node":
        self._app._set_handler(self.id, event, None)
        return self

    def focus(self) -> "Node":
        """Gives the node the keyboard focus on the next commit (an Input, a Textarea, a Command, a div with keyDown…)."""
        self._app._pending.append({"op": "focus", "id": self.id})
        return self

    def remove(self) -> None:
        """Removes the node and all its descendants."""
        self._app._remove(self.id)


class Window:
    """A window other than the main one (`App.create_window`)."""

    def __init__(self, app: "App", window_id: int):
        self._app = app
        self.id = window_id
        self.on_closed = None  # () -> None; the user closed the window
        self._handlers: dict = {}  # window event -> handler
        self._events_sent: list[str] = []

    def set(self, *, root: Node | None = None, **options) -> "Window":
        """Updates the window; like `App.window`, it opens on the first commit that sets `root`."""
        self._app._queue_window(self.id, root, options)
        return self

    def close(self) -> None:
        """Closes the window on the next commit. `on_closed` is not called: it is for the user closing it."""
        self._app._pending.append({"op": "closeWindow", "id": self.id})
        self._app._open_windows.discard(self.id)

    def on(self, event: str, handler) -> "Window":
        """Subscribes to a window event (`focus`, `blur`, `resize`, `move`, `visibility`) or to
        `closed`. Handlers get the message: bounds for `move`/`resize`, `visible` for `visibility`."""
        if event == "closed":
            self.on_closed = handler
            return self
        if event not in WINDOW_EVENTS:
            expected = ", ".join(("closed", *WINDOW_EVENTS))
            raise ValueError(f"unknown window event '{event}' (expected one of {expected})")
        self._handlers[event] = handler
        self._sync_events()
        return self

    def off(self, event: str) -> "Window":
        """Unsubscribes; the last one removes the window's event subscription (no messages follow)."""
        if event == "closed":
            self.on_closed = None
            return self
        self._handlers.pop(event, None)
        self._sync_events()
        return self

    def _sync_events(self) -> None:
        """Sends the subscription list only when the set of subscribed events changes."""
        events = list(self._handlers)
        if events != self._events_sent:
            self._events_sent = events
            self._app._queue_window(self.id, None, {"events": events})

    def _dispatch_window_event(self, message: dict) -> None:
        handler = self._handlers.get(message.get("event"))
        if handler:
            handler(message)


class App:
    """Starts the host and completes the handshake. Raises with a clear message if the host cannot start."""

    def __init__(self, host_path: str | None = None, *, debug: bool = False, headless: bool = False):
        self.on_error = None  # (HostError) -> None; without it errors are printed to stderr
        self.on_window_closed = None  # (window_id) -> None; the user closed a window
        self.on_window_event = None  # (message) -> None; a window event the app subscribed to
        self.on_tray = None  # (event, item) -> None; tray "click" (item None) or "menu" (item id)
        self.on_shortcut = None  # (keys, window_id) -> None; a key registered with shortcuts()
        self.on_notification = None  # (id, window_id) -> None; the user clicked a toast that has an id
        self._has_tray = False  # a tray keeps the host running after the last window closes
        self._next_request = 1
        self._results: dict[int, dict] = {}  # answers that arrived while waiting for another one
        self._next_id = 1
        self._nodes: dict[int, _Record] = {}
        self._pending: list[dict] = []
        self._inbox: queue.Queue = queue.Queue()
        self._stderr_tail: deque[str] = deque(maxlen=STDERR_TAIL_LINES)
        self._exit: HostExit | None = None
        self._closing = False
        self._next_window_id = 2
        self._windows: dict[int, Window] = {}
        # Windows given a root and not closed: the host exits when the user closes the last one.
        self._open_windows: set[int] = set()

        path = host_path or default_host_path()
        args = [path, "--no-window"] if headless else [path]
        try:
            self._proc = subprocess.Popen(args, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        except FileNotFoundError:
            raise HostExitedError(
                f"gpui-bridge host not found at {path}. Build it with `cargo build` in host/ or pass host_path.",
                HostExit(None, []),
            ) from None

        self._stderr_thread = threading.Thread(target=self._read_stderr, args=(debug,), daemon=True)
        self._stderr_thread.start()
        threading.Thread(target=self._read_stdout, daemon=True).start()
        self._write([{"op": "hello", "protocol": PROTOCOL_VERSION}])
        self._handshake()

    def div(self, **props) -> Node:
        return self._create("div", props)

    def text(self, **props) -> Node:
        return self._create("text", props)

    def button(self, **props) -> Node:
        return self._create("Button", props)

    def input(self, **props) -> Node:
        return self._create("Input", props)

    def checkbox(self, **props) -> Node:
        return self._create("Checkbox", props)

    def icon(self, **props) -> Node:
        return self._create("Icon", props)

    def image(self, **props) -> Node:
        """An image from `src` (a local file) or `data` (standard base64 of the bytes,
        for images the app downloads itself). `fit` is the CSS `object-fit`."""
        return self._create("Image", props)

    def switch(self, **props) -> Node:
        return self._create("Switch", props)

    def radio_group(self, **props) -> Node:
        """`options=[{"value": ..., "label": ...}]`, `value`, `direction`."""
        return self._create("RadioGroup", props)

    def select(self, **props) -> Node:
        """`options=[{"value": ..., "label": ...}]`, `value`, `placeholder`, `searchable`."""
        return self._create("Select", props)

    def slider(self, **props) -> Node:
        """`value`, `min` (0), `max` (100), `step` (1); `change` passes a number."""
        return self._create("Slider", props)

    def textarea(self, **props) -> Node:
        return self._create("Textarea", props)

    def dialog(self, **props) -> Node:
        """A modal dialog shown while `open=True`; append its body as children. Events `ok`, `cancel`."""
        return self._create("Dialog", props)

    def virtual_list(self, **props) -> Node:
        """A list that draws only the rows in view (PROTOCOL.md section 15): append one child per
        row; `rowHeight` (32). Give it a height (`class_="flex-1"`)."""
        return self._create("VirtualList", props)

    def table(self, **props) -> Node:
        """A data table with text cells: `columns=[{"key": ..., "name": ...}]`, `rows=[{key: cell}]`,
        `selected`, `stripe`, `bordered`. `change` passes a row index (None when cleared),
        `doubleClick` a row index, `sort` a dict `{"column": ..., "direction": "asc" | "desc" | "none"}`."""
        return self._create("Table", props)

    def tabs(self, **props) -> Node:
        """A tab bar: `tabs=[{"value": ..., "label": ...}]`, `value`, `variant`. Show the content yourself."""
        return self._create("Tabs", props)

    def tab(self, **props) -> Node:
        """One tab of a `tabs` bar: `value` (required, not empty), `label` (its text, before the
        children), `disabled` and `class` (of the chip). Append the nodes of its chip (an `icon`, a
        `text`, a `tag`; a last `button` is drawn after them); a `Tabs` with `tab` children uses
        them and ignores `tabs`."""
        return self._create("Tab", props)

    def menu_bar(self, **props) -> Node:
        """A menu bar (PROTOCOL.md section 10): `menus=[{"label": ..., "items": [...]}]`."""
        return self._create("MenuBar", props)

    # gpui-kit's Basic, Form, Layout and Advanced groups (PROTOCOL.md section 17).

    def alert(self, **props) -> Node:
        """An inline message: `message`, `title`, `variant`, `banner`, `closable`. Event `close`."""
        return self._create("Alert", props)

    def badge(self, **props) -> Node:
        """A count (`count`, `max`), a `dot` or an `icon`, over its children."""
        return self._create("Badge", props)

    def tag(self, **props) -> Node:
        """`label`, `variant` (primary, secondary, danger, success, warning, info), `outline`, `icon`."""
        return self._create("Tag", props)

    def kbd(self, **props) -> Node:
        """A key or shortcut, such as `keys="ctrl-s"`; `outline`."""
        return self._create("Kbd", props)

    def progress(self, **props) -> Node:
        """A bar: `value` from 0 to 100, `loading`."""
        return self._create("Progress", props)

    def spinner(self, **props) -> Node:
        """A spinner; size it with `class_` (`size-4`)."""
        return self._create("Spinner", props)

    def skeleton(self, **props) -> Node:
        """A loading placeholder; size it with `class_`."""
        return self._create("Skeleton", props)

    def rating(self, **props) -> Node:
        """Stars: `value`, `max` (5), `controlled`. `change` passes the chosen count."""
        return self._create("Rating", props)

    def pagination(self, **props) -> Node:
        """Pages: `page` (1-based), `total`, `compact`, `controlled`. `change` passes the page."""
        return self._create("Pagination", props)

    def collapsible(self, **props) -> Node:
        """Content shown while `open`; append it as children."""
        return self._create("Collapsible", props)

    def accordion(self, **props) -> Node:
        """Sections as `items=[{"id", "title", "content", "open", "disabled"}]`, `multiple`,
        `bordered`, `controlled`. `change` passes the id of the section toggled."""
        return self._create("Accordion", props)

    def avatar(self, **props) -> Node:
        """A picture from `src`, or the initial of `name`."""
        return self._create("Avatar", props)

    def stepper(self, **props) -> Node:
        """Steps as `items=[{"value", "label"}]`, `value`, `vertical`, `controlled`.
        `change` passes the chosen step's value."""
        return self._create("Stepper", props)

    def number_input(self, **props) -> Node:
        """A text field with increment and decrement buttons: `value`, `placeholder`, `controlled`."""
        return self._create("NumberInput", props)

    def otp_input(self, **props) -> Node:
        """A one-time code: `length` (6), `value`, `masked`, `controlled`."""
        return self._create("OtpInput", props)

    def form(self, **props) -> Node:
        """A form: `direction` ("vertical" or "horizontal"); append `field` nodes."""
        return self._create("Form", props)

    def field(self, **props) -> Node:
        """A label, an optional `description`, and its children as the control."""
        return self._create("Field", props)

    def group_box(self, **props) -> Node:
        """A titled box: `title`, `variant` (normal, fill, outline); append its content."""
        return self._create("GroupBox", props)

    def description_list(self, **props) -> Node:
        """Key-value rows: `items=[{"label", "value", "span"}]`, `columns`, `bordered`, `horizontal`."""
        return self._create("DescriptionList", props)

    def status_bar(self, **props) -> Node:
        """A status bar: `left` and `right` text."""
        return self._create("StatusBar", props)

    def toolbar(self, **props) -> Node:
        """A toolbar; append its content."""
        return self._create("Toolbar", props)

    def resizable(self, **props) -> Node:
        """Panels the user can resize: `axis` (horizontal or vertical), `sizes` in pixels. Append
        one child per panel."""
        return self._create("Resizable", props)

    def calendar(self, **props) -> Node:
        """A month view: `value` ("YYYY-MM-DD"), `months`, `weekStartsOn`, `controlled`.
        `change` passes the chosen day."""
        return self._create("Calendar", props)

    def tree(self, **props) -> Node:
        """Items in props: `items=[{"id", "label", "children", "expanded", "disabled"}]`, `value`
        (the selected item id), `controlled`. `change` passes the chosen id; `expand` and
        `collapse` pass a folder's id."""
        return self._create("Tree", props)

    def diff(self, **props) -> Node:
        """A patch: `patch` (a unified or Git diff), `mode` (unified or split), `lineNumbers`,
        `softWrap`, `header`. `change` passes the selected lines ({path, side, start, end,
        endSide}) or None."""
        return self._create("Diff", props)

    def carousel(self, **props) -> Node:
        """A carousel: `value` (the selected slide index), `axis`, `looping`, `controls`,
        `pagination`, `controlled`. Append one child per slide; `change` passes the index."""
        return self._create("Carousel", props)

    def sidebar(self, **props) -> Node:
        """A sidebar: `items` (sections: {"label", "items": [{"id", "label", "icon",
        "disabled", "open", "children"}]}), `value` (the active entry id), `collapsed`,
        `collapsible` (icon, offcanvas, none), `side` (left, right), `controlled`. `change`
        passes the clicked entry's id."""
        return self._create("Sidebar", props)

    def chart(self, **props) -> Node:
        """A chart: `chart` (line, bar, pie), `data` ([{"label", "value"}]), `color`, `name`,
        plus the options of each plot (`curve`, `dot`, `horizontal`, `xAxis`, `xTicks`, `yAxis`, `yTicks`, `grid`,
        `yMin`, `yMax`, `innerRadius`, `padAngle`, `interactive`). It has no events."""
        return self._create("Chart", props)

    def canvas(self, **props) -> Node:
        """A canvas: `commands`, painted in order ({"op": "rect", x, y, w, h, color,
        corner_radius}, {"op": "line", x1, y1, x2, y2, color, width}, {"op": "path", d, color,
        width}). `pointerDown` passes the local point ({"x", "y"})."""
        return self._create("Canvas", props)

    def command(self, **props) -> Node:
        """A command palette: `items` (sections: {"label", "items": [{"id", "label", "icon",
        "keywords", "checked", "disabled"}]}), `placeholder`. It filters by the query itself;
        `confirm` passes the chosen entry's id."""
        return self._create("Command", props)

    def settings(self, **props) -> Node:
        """Settings pages: `pages` ([{"title", "icon", "groups": [{"title", "items": [{"key",
        "label", "description", "type", "value", "options", "min", "max", "step", "disabled"}]}]}]),
        with `type` one of switch, checkbox, input, dropdown, number. The host keeps each value the
        user sets; `change` passes {"key", "value"}."""
        return self._create("Settings", props)

    def toggle(self, **props) -> Node:
        """A button that stays pressed: `checked`, `label`, `icon`, `variant` (ghost, outline),
        `tooltip`, `disabled`, `controlled`. `change` passes the new `checked`."""
        return self._create("Toggle", props)

    def dropdown_button(self, **props) -> Node:
        """An action with a menu: `label`, `icon`, `variant`, `outline`, `tooltip`, `disabled`,
        `items` (a menu list) and `menuLabel`. `click` is the left half; `menu` passes the chosen item's id."""
        return self._create("DropdownButton", props)

    def label(self, **props) -> Node:
        """Text: `content`, `secondary` (a muted text after it), `masked`, `highlight` (marks
        every occurrence of the substring, ignoring case)."""
        return self._create("Label", props)

    def empty(self, **props) -> Node:
        """A centered empty state: `icon`, `title`, `description`; append the action (a button)."""
        return self._create("Empty", props)

    def shimmer(self, **props) -> Node:
        """Loading text: `content`, `duration` in seconds, `spread`, `reverse`, `once`."""
        return self._create("Shimmer", props)

    def input_group(self, **props) -> Node:
        """A framed text field: `value`, `placeholder`, `prefix`, `suffix`, `prefixIcon`,
        `suffixIcon`, `disabled`, `readonly`, `invalid`, `controlled`. `change` and `submit` pass
        the text, like an `Input`."""
        return self._create("InputGroup", props)

    def clipboard(self, **props) -> Node:
        """Copies `value` on click: `tooltip`. `copied` passes the text that was copied."""
        return self._create("Clipboard", props)

    # Form controls with state (PROTOCOL.md section 25).

    def date_picker(self, **props) -> Node:
        """A day picker: `value` ("YYYY-MM-DD", or "YYYY-MM-DD/YYYY-MM-DD" with `range`),
        `range`, `placeholder`, `disabled`, `controlled`. `change` passes the chosen day
        (None when cleared)."""
        return self._create("DatePicker", props)

    def time_field(self, **props) -> Node:
        """A time typed by segments: `value` ("HH:MM", or "HH:MM:SS" with `seconds`),
        `seconds`, `disabled`, `controlled`. It has no empty state; `change` passes the time."""
        return self._create("TimeField", props)

    def color_picker(self, **props) -> Node:
        """A color swatch and hex box: `value` ("#RRGGBB" or "#RRGGBBAA", `#` required),
        `palette`, `controlled`. No `disabled`. `change` passes the color (None when cleared)."""
        return self._create("ColorPicker", props)

    def combobox(self, **props) -> Node:
        """A list that picks several options: `options` ([{"value", "label", "disabled"}]),
        `value` (the list of chosen values), `placeholder`, `searchable`, `disabled`,
        `controlled`. `change` passes the whole selection."""
        return self._create("Combobox", props)

    # Chat (PROTOCOL.md section 26).

    def bubble(self, **props) -> Node:
        """A chat bubble: `variant`, `alignment`. The children are the content."""
        return self._create("Bubble", props)

    def message(self, **props) -> Node:
        """A message row: `alignment`, `avatarName`, `avatarSrc`. A `message_header` and a
        `message_footer` child are its header and footer; the other children are the content."""
        return self._create("Message", props)

    def message_header(self, **props) -> Node:
        """A `message`'s header (sender, time); outside a message it draws nothing."""
        return self._create("MessageHeader", props)

    def message_footer(self, **props) -> Node:
        """A `message`'s footer (state, actions); outside a message it draws nothing."""
        return self._create("MessageFooter", props)

    def marker(self, **props) -> Node:
        """A status row: `content`, `icon`, `variant`, `alignment`, `loading`, `loadingStyle`."""
        return self._create("Marker", props)

    def attachment(self, **props) -> Node:
        """A file or picture card: `status`, `progress`, `title`, `description`, `tooltip`,
        `src`/`data`, `axis`. Subscribing to `click`, `remove` or `retry` shows that control
        (`retry` only while `failed`); none passes a value. The children are actions."""
        return self._create("Attachment", props)

    def message_scroller(self, **props) -> Node:
        """A virtual list of messages that stays at the end: `scrollbar`, `jumpButton`,
        `jumpLabel`. Each child is a row."""
        return self._create("MessageScroller", props)

    def questionnaire(self, **props) -> Node:
        """Questions one at a time: `items` ([{"name", "label", "choices", "input", ...}]).
        `change` passes one answer ({"name", "status", "choices", "text"}); `submit` passes
        the list of answers."""
        return self._create("Questionnaire", props)

    def alert_dialog(self, **props) -> Node:
        """A confirmation: `open`, `title`, `description`, `icon`, `okText`, `cancelText`. The
        children are the body. `ok` and `cancel` close it, like a `Dialog`, and it shows over one."""
        return self._create("AlertDialog", props)

    def sheet(self, **props) -> Node:
        """A panel from `side` (left, right, top, bottom): `open`, `title`, `size` in pixels. The
        children are the scrollable body. `close` is its X, Escape or a click on the overlay."""
        return self._create("Sheet", props)

    def popover(self, **props) -> Node:
        """A panel anchored to the first child (the trigger): `open`, `anchor` (top-left,
        top-center, top-right, left-center, right-center, bottom-left, bottom-center,
        bottom-right), `arrow`. `change` passes the new `open`, and the host keeps it."""
        return self._create("Popover", props)

    def hover_card(self, **props) -> Node:
        """A card on hover: `openDelay` and `closeDelay` in milliseconds, `anchor`. The mouse is
        the only way in (there is no `open`); `change` passes the new state."""
        return self._create("HoverCard", props)

    def window(self, *, root: Node | None = None, **options) -> "App":
        """Creates or updates the main window (window 1). It opens on the first commit that sets `root`."""
        self._queue_window(None, root, options)
        return self

    def create_window(self, *, root: Node | None = None, **options) -> Window:
        """Another window, with its own root; it opens on the first commit that sets `root`."""
        window = Window(self, self._next_window_id)
        self._next_window_id += 1
        self._windows[window.id] = window
        return window.set(root=root, **options)

    def _queue_window(self, window_id: int | None, root: Node | None, options: dict) -> None:
        message = {"op": "window", **({"id": window_id} if window_id is not None else {}), **options}
        if root:
            message["root"] = root.id
            self._open_windows.add(window_id or 1)
        self._pending.append(message)

    def open_dialog(self, *, files: bool = True, directories: bool = False, multiple: bool = False, prompt: str | None = None):
        """Native "open" dialog: the chosen paths, or None if cancelled. Handlers keep running meanwhile."""
        options = {"files": files, "directories": directories, "multiple": multiple, **({"prompt": prompt} if prompt else {})}
        return self._request({"op": "openDialog", **options}, "paths")

    def save_dialog(self, *, directory: str | None = None, name: str | None = None):
        """Native "save" dialog: the chosen path, or None if cancelled."""
        options = {key: value for key, value in (("directory", directory), ("name", name)) if value is not None}
        return self._request({"op": "saveDialog", **options}, "path")

    def read_clipboard(self):
        """The clipboard's text, or None if it holds none."""
        return self._request({"op": "readClipboard"}, "text")

    def notify(self, message: str, **options) -> None:
        """Shows a toast now: `title`, `type`, `window`, `id`, `autohide`, `system`."""
        self._write([{"op": "notify", "message": message, **options}])

    def write_clipboard(self, text: str) -> None:
        self._write([{"op": "writeClipboard", "text": text}])

    def open_url(self, url: str) -> None:
        """Opens a URL with the system's handler (the browser for http)."""
        self._write([{"op": "openUrl", "url": url}])

    def open_path(self, path: str) -> None:
        """Opens a file or folder with its default app."""
        self._write([{"op": "openPath", "path": path}])

    def _request(self, message: dict, key: str):
        """Sends a system action and waits for its result, dispatching other messages meanwhile."""
        if self._exit:
            raise HostExitedError(_describe_exit(self._exit), self._exit)
        request = self._next_request
        self._next_request += 1
        self._write([{**message, "request": request}])
        while request not in self._results:
            try:
                message = self._inbox.get(timeout=0.5)
            except queue.Empty:
                continue
            if isinstance(message, HostExit):
                self._inbox.put(message)  # left for run()
                raise HostExitedError(_describe_exit(message), message)
            if message.get("type") in ("result", "error") and message.get("request") is not None:
                self._results[message.get("request")] = message
            else:
                self._dispatch(message)
        answer = self._results.pop(request)
        if answer.get("type") == "error":
            raise RuntimeError(f"gpui-bridge: host rejected '{answer.get('op')}': {answer.get('message')}")
        return answer.get(key)

    def shortcuts(self, keys: list[str]) -> "App":
        """App-wide keyboard shortcuts ("ctrl-s", "f5"...), applied on the next commit; replaces the list."""
        self._pending.append({"op": "shortcuts", "keys": list(keys)})
        return self

    def tray(self, **options) -> "App":
        """Shows or updates the system tray icon (PROTOCOL.md section 11): `icon`, `tooltip`, `menu`."""
        self._pending.append({"op": "tray", **options})
        self._has_tray = True
        return self

    def remove_tray(self) -> "App":
        """Removes the tray icon on the next commit; with no window open, the host then exits."""
        self._pending.append({"op": "removeTray"})
        self._has_tray = False
        if not self._open_windows:
            self._closing = True
        return self

    def commit(self) -> None:
        """Sends every pending operation; the host renders once per commit."""
        if self._exit:
            raise HostExitedError(_describe_exit(self._exit), self._exit)
        self._write([*self._pending, {"op": "commit"}])
        self._pending.clear()

    def run(self) -> HostExit:
        """Calls event handlers until the host exits (window closed, `close()` or crash)."""
        try:
            while self._exit is None:
                try:
                    # A timeout keeps Ctrl+C responsive on Windows.
                    message = self._inbox.get(timeout=0.5)
                except queue.Empty:
                    continue
                if isinstance(message, HostExit):
                    self._exit = message
                    try:
                        self._proc.stdin.close()
                    except OSError:
                        pass  # unflushed data to a dead host
                    # An exit after close() or windowClosed is expected; anything else is a crash.
                    if not self._closing:
                        print(_describe_exit(message), file=sys.stderr)
                else:
                    self._dispatch(message)
        except BaseException:
            self._proc.kill()
            raise
        return self._exit

    def close(self) -> HostExit:
        """Closes stdin (the host exits on EOF), handles remaining messages and waits for the exit."""
        self._closing = True
        try:
            self._proc.stdin.close()
        except OSError:
            pass
        return self.run()

    def _create(self, type: str, props: dict) -> Node:
        # Ids are positive, unique for the whole session and never reused.
        id = self._next_id
        self._next_id += 1
        self._nodes[id] = _Record(type)
        self._pending.append({"op": "create", "id": id, "type": type, "props": _props(props)})
        return Node(self, id, type)

    def _attach(self, parent: int, child: int) -> None:
        # Mirrors the host's move semantics so `remove` can drop descendant handlers.
        record = self._nodes.get(child)
        if not record:
            return
        if record.parent is not None and record.parent in self._nodes:
            self._nodes[record.parent].children.discard(child)
        record.parent = parent
        if parent in self._nodes:
            self._nodes[parent].children.add(child)

    def _remove(self, id: int) -> None:
        record = self._nodes.get(id)
        if not record:
            return
        if record.parent is not None and record.parent in self._nodes:
            self._nodes[record.parent].children.discard(id)
        stack = [id]
        while stack:
            node = self._nodes.pop(stack.pop(), None)
            if node:
                stack.extend(node.children)
        self._pending.append({"op": "remove", "id": id})

    def _set_handler(self, id: int, event: str, handler) -> None:
        # Queues `setEvents` only when the subscription set changes.
        record = self._nodes.get(id)
        if not record:
            return
        had = event in record.handlers
        if handler:
            record.handlers[event] = handler
        else:
            record.handlers.pop(event, None)
        if had != (event in record.handlers):
            self._pending.append({"op": "setEvents", "id": id, "events": list(record.handlers)})

    def _dispatch(self, message: dict) -> None:
        match message.get("type"):
            case "event":
                record = self._nodes.get(message.get("id"))
                handler = record and record.handlers.get(message.get("event"))
                if not handler:
                    return
                # A menu choice passes the item id; a Slider, a number.
                if record.type in ("Checkbox", "Switch", "Toggle", "Popover", "HoverCard"):
                    handler(message.get("checked", False))
                elif "paths" in message:
                    handler(message["paths"])
                elif "value" in message or record.type in ("Input", "Textarea"):
                    handler(message.get("value", ""))
                else:
                    handler()
            case "error":
                error = HostError(message.get("code"), message.get("message"), message.get("op"))
                if self.on_error:
                    self.on_error(error)
                else:
                    print(f"gpui-bridge: host rejected '{error.op}': {error.message}", file=sys.stderr)
            case "windowClosed":
                window_id = message.get("id", 1)
                self._open_windows.discard(window_id)
                # The host exits after the last window, unless there is a tray.
                if not self._open_windows and not self._has_tray:
                    self._closing = True
                window = self._windows.get(window_id)
                if window and window.on_closed:
                    window.on_closed()
                if self.on_window_closed:
                    self.on_window_closed(window_id)
            case "windowEvent":
                window = self._windows.get(message.get("id", 1))
                if window:
                    window._dispatch_window_event(message)
                if self.on_window_event:
                    self.on_window_event(message)
            case "notification":
                if self.on_notification:
                    self.on_notification(message.get("id"), message.get("window"))
            case "shortcut":
                if self.on_shortcut:
                    self.on_shortcut(message.get("keys"), message.get("window"))
            case "tray":
                if self.on_tray:
                    self.on_tray(message.get("event"), message.get("value"))

    def _handshake(self) -> None:
        deadline = time.monotonic() + HANDSHAKE_TIMEOUT
        while True:
            try:
                message = self._inbox.get(timeout=max(0.0, deadline - time.monotonic()))
            except queue.Empty:
                self._proc.kill()
                raise TimeoutError(f"gpui-bridge host did not answer the handshake within {HANDSHAKE_TIMEOUT:g} s") from None
            if isinstance(message, HostExit):
                raise HostExitedError(_describe_exit(message), message)
            if message.get("type") == "ready":
                return
            if message.get("type") == "error":
                raise RuntimeError(f"gpui-bridge handshake failed ({message.get('code')}): {message.get('message')}")

    def _write(self, messages: list[dict]) -> None:
        if self._proc.stdin.closed:
            return
        try:
            for message in messages:
                self._proc.stdin.write(json.dumps(message, separators=(",", ":")).encode() + b"\n")
            self._proc.stdin.flush()
        except OSError:
            pass  # Writes racing a dying host fail; run() reports the real cause.

    def _read_stdout(self) -> None:
        for raw in self._proc.stdout:
            line = raw.decode("utf-8", "replace").rstrip("\r\n")
            try:
                message = json.loads(line)
            except ValueError:
                message = None
            if isinstance(message, dict):
                self._inbox.put(message)
            else:
                print(f"gpui-bridge: host wrote a non-protocol line to stdout: {line}", file=sys.stderr)
        self._proc.stdout.close()
        # Wait for stderr too, so the exit carries the complete tail.
        self._stderr_thread.join()
        self._inbox.put(HostExit(self._proc.wait(), list(self._stderr_tail)))

    def _read_stderr(self, debug: bool) -> None:
        for raw in self._proc.stderr:
            line = raw.decode("utf-8", "replace").rstrip("\r\n")
            self._stderr_tail.append(line)
            if debug:
                print(f"[host] {line}", file=sys.stderr)
        self._proc.stderr.close()
