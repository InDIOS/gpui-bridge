# gpui-bridge protocol (version 1)

How a client process drives the gpui-bridge host. The protocol is language-neutral:
any language that can spawn a process and read and write lines can be a client
(today: Node, txiki.js. Go and Python, all in this repository).

The host is a child process of the client. It owns the window, the event loop, the
component tree and rendering. The client describes the UI as **operations on a tree**
and receives **events** back.

## 1. Transport

- Encoding: **JSON Lines**, UTF-8. One message per line, terminated by `\n`.
- Client → host: the host's stdin. Host → client: the host's stdout.
- Client messages use the key `op`. Host messages use the key `type`.
- The host's stderr carries logs only; a client may forward them to its console in debug mode.
- The host exits when its stdin reaches EOF (the client went away).
- `--no-window`: the host runs the same protocol without opening a window and dumps the
  tree to stderr after each commit. Used for tests and the conformance fixtures (section 9).

## 2. Handshake

The client sends first:

```json
{"op":"hello","protocol":1}
```

The host answers:

```json
{"type":"ready","protocol":1}
```

If the version is not supported, the host answers
`{"type":"error","code":"protocol_mismatch","op":"hello",...}` and exits.

Before the handshake, the host ignores everything that is not `hello` and does not answer
(it only logs a warning to stderr). Until the version is agreed, no other message can be
interpreted safely.

## 3. Windows

```json
{"op":"window","title":"My app","width":800,"height":600,"root":1}
{"op":"window","id":2,"title":"Settings","width":400,"height":300,"root":50}
{"op":"closeWindow","id":2}
```

`window` creates (or updates) a window. `root` is the id of its root node.

- `id` (optional): the window, chosen by the client like node ids (a positive integer).
  Without `id` the message is about window 1, the main window, so a single-window client
  never sends one. `id: 0` is `invalid_operation`.
- Every field is optional; an update sends only what changes. A window opens on the first
  `commit` that defines its `root`. New windows open centered.
- A node is the root of one window at a time: using another window's root is
  `invalid_operation`. Keep each window's nodes in its own tree; a node placed in two
  windows' trees is not supported.
- `closeWindow` closes a window on the next `commit` and forgets it, so the same `id` can
  open again later. It sends no `windowClosed` (section 6). An unknown `id` is
  `invalid_operation`.
- The host exits when the user closes the last open window, unless there is a tray icon
  (section 11). Closing window 1 does not end the app while other windows are open; closing
  all of them with `closeWindow` does not either (the host waits for a new `window`).
- `theme`, `themeSet`: the theme is app-wide. Whichever window message carries them, they
  apply to every window.
- `theme` (optional): `"light"` or `"dark"`, the gpui-component theme mode. Light when
  absent. It can change at any time and every window repaints.
- `frameless` (optional): `true` opens the window without the system title bar. The window
  keeps its resize borders, shadow and taskbar title; the app draws its own title bar with
  `windowControl` divs (section 5). Read only when the window opens: a later change is
  ignored, with a warning on stderr.
- `x`, `y` (optional): the window's initial position, in logical pixels, from the top-left
  corner of the screen. Read only when the window opens (a later change warns on stderr).
  With neither, the window opens centered; with only one, the other is 0.
- `minWidth`, `minHeight` (optional): the smallest size the user can resize the window to.
  Read only when the window opens; a missing one leaves that axis unconstrained.
- `minimized` (optional): `true` minimizes the window on the next `commit`, `false` restores
  it and brings it to the front. It is an **action, not a remembered state**: send it again
  to act again, and the user can restore or minimize the window at any time. A window that is
  not open is not an error; the host warns on stderr. GPUI (0.3.9) has no "hide" for an open
  window, so `minimized` is the way to take one out of sight without closing it.
- `events` (optional): the window events the client wants (see "Window events" below). It
  **replaces** the previous list, so `[]` unsubscribes, and an unknown name is `invalid_json`.
  Like node events, they are opt-in; `windowClosed` is always sent.
- `themeSet` (optional): a gpui-component theme file, used in place of the default theme.
  `theme` picks its first theme with that `mode`; a mode it does not define uses the default
  theme. Each `themeSet` replaces the previous one, so `{"themes":[]}` restores the defaults.

  ```json
  {"op":"window","theme":"dark","themeSet":{"name":"Brand","themes":[
    {"name":"Brand Dark","mode":"dark","radius":12,"font_family":"Inter",
     "colors":{"background":"#1c0a00","primary.background":"#f97316"}}]}}
  ```

  The format is gpui-component's (0.7), with its own field names: `font_size`,
  `font_family`, `mono_font_size`, `mono_font_family`, `radius`, `radius_lg`, `shadow`
  and `colors`. Color keys are dotted (`primary.background`, `primary.foreground`,
  `danger.background`, `border`…); values are `#rrggbb[aa]` or palette names such as
  `neutral-200`. A missing field uses gpui-component's built-in value, and so does a color
  value that does not parse, without an error. A field with the wrong type, or a `mode`
  other than `light`/`dark`, rejects the message with `invalid_json`.

### Window events

```json
{"op":"window","id":2,"events":["focus","resize"]}
{"type":"windowEvent","id":2,"event":"focus"}
{"type":"windowEvent","id":2,"event":"resize","x":120,"y":80,"width":400,"height":300}
{"type":"windowEvent","id":2,"event":"move","x":160,"y":80,"width":400,"height":300}
{"type":"windowEvent","id":2,"event":"visibility","visible":false}
```

| event | when | fields |
|---|---|---|
| `focus` | the window gained the system focus (an opening window reports it too) | — |
| `blur` | the window lost it | — |
| `resize` | the size changed, by the user or by `width`/`height` | `x`, `y`, `width`, `height` |
| `move` | only the position changed | `x`, `y`, `width`, `height` |
| `visibility` | the platform stopped or resumed presenting the window (minimizing, covering it, display sleep) | `visible` |

Only changes are sent: opening a window does not report its initial size or position (they
are the ones the client asked for). `resize` and `move` carry the full bounds, in the same
coordinates `x`, `y`, `width` and `height` take — the window frame's top-left and the client
size — so a client can feed a reported `x`, `y`, `width`, `height` straight back into `window`
to restore it. While the window is minimized the platform moves it off screen; the host does
not report that as a `move`. They are sent while the user drags or resizes, at the
platform's pace; a client that only needs the final size can debounce. Events for a window
whose `events` list does not include them are not sent at all.

## 4. Tree operations

```json
{"op":"create","id":1,"type":"div","props":{"class":"flex flex-col gap-2 p-4"}}
{"op":"create","id":2,"type":"text","props":{"content":"Hello"}}
{"op":"create","id":3,"type":"Button","props":{"label":"Save","variant":"primary"},"events":["click"]}
{"op":"append","parent":1,"child":2}
{"op":"insertBefore","parent":1,"child":3,"before":2}
{"op":"setProps","id":3,"props":{"disabled":true}}
{"op":"setEvents","id":3,"events":["click"]}
{"op":"remove","id":3}
{"op":"commit"}
```

Rules:

- **The client assigns ids.** Positive integers, unique for the whole session, never reused.
- `setProps` **merges** props. To remove a prop, send it as `null`.
- `remove` deletes the node and **all its descendants**.
- Operations accumulate and **are applied on `commit`**. The host only re-renders after a commit.
- `events` is opt-in: the host only emits events the node asked for. `setEvents` replaces the set.
- `append` and `insertBefore` on a node that already has a parent **move** it, as in the DOM.
  A node cannot be placed inside itself or one of its descendants.
- On `commit`, an operation that fails is dropped and answered with an `error`; the rest of
  the batch is still applied, in order.
- A rejected `create` leaves no node: later operations that name its id get `unknown_node`.

## 5. Node types

| type       | props                                                        | events             |
|------------|--------------------------------------------------------------|--------------------|
| `div`      | `class`, `windowControl` (`drag`, `minimize`, `maximize`, `close`), `contextMenu`, `tooltip` | `click`, `doubleClick`, `menu`, `keyDown`, `drop` |
| `text`     | `content`, `class`                                           | —                  |
| `Button`   | `label`, `variant` (`primary`, `secondary`, `ghost`, `danger`), `disabled`, `tooltip`, `class` | `click` |
| `Input`    | `value`, `placeholder`, `disabled`, `controlled`, `class`    | `change`, `submit`, `keyDown` |
| `Checkbox` | `checked`, `label`, `disabled`, `controlled`, `class`        | `change`           |
| `Icon`     | `name`, `class`                                              | —                  |
| `Image`    | `src`, `data`, `fit` (`contain`, `cover`, `fill`, `scale-down`, `none`), `class` | — |
| `MenuBar`  | `menus`, `class`                                             | `menu`             |
| `Switch`   | `checked`, `label`, `disabled`, `controlled`, `class`        | `change`           |
| `RadioGroup` | `options`, `value`, `direction` (`vertical`, `horizontal`), `disabled`, `controlled`, `class` | `change` |
| `Select`   | `options`, `value`, `placeholder`, `searchable`, `disabled`, `controlled`, `class` | `change` |
| `Slider`   | `value`, `min`, `max`, `step`, `disabled`, `controlled`, `class` | `change`        |
| `Textarea` | `value`, `placeholder`, `disabled`, `class`                  | `change`           |
| `Dialog`   | `open`, `title`, `okText`, `cancelText`, `width`, `class`    | `ok`, `cancel`     |
| `VirtualList` | `rowHeight`, `class`                                      | —                  |
| `Table`    | `columns`, `rows`, `selected`, `stripe`, `bordered`, `controlled`, `class` | `change`, `doubleClick`, `sort` |
| `Tabs`     | `tabs`, `value`, `variant`, `controlled`, `class`            | `change`           |
| `Tab`      | `value`, `label`, `disabled`, `class`                        | —                  |
| `Alert`    | `message`, `title`, `variant`, `banner`, `closable`, `class` | `close`            |
| `Badge`    | `count`, `max`, `dot`, `icon`, `class`                       | —                  |
| `Tag`      | `label`, `variant`, `outline`, `icon`, `class`               | —                  |
| `Kbd`      | `keys`, `outline`, `class`                                   | —                  |
| `Progress` | `value`, `loading`, `class`                                  | —                  |
| `Spinner`  | `class`                                                      | —                  |
| `Skeleton` | `secondary`, `class`                                         | —                  |
| `Rating`   | `value`, `max`, `disabled`, `controlled`, `class`            | `change`           |
| `Pagination` | `page`, `total`, `compact`, `disabled`, `controlled`, `class` | `change`         |
| `Collapsible` | `open`, `class`                                           | —                  |
| `Accordion` | `items`, `multiple`, `bordered`, `disabled`, `controlled`, `class` | `change`     |
| `Avatar`   | `name`, `src`, `class`                                       | —                  |
| `Stepper`  | `items`, `value`, `vertical`, `disabled`, `controlled`, `class` | `change`        |
| `NumberInput` | `value`, `placeholder`, `disabled`, `controlled`, `class` | `change`, `submit`, `keyDown` |
| `OtpInput` | `length`, `value`, `masked`, `disabled`, `controlled`, `class` | `change`         |
| `Form`     | `direction`, `class`                                         | —                  |
| `Field`    | `label`, `description`, `required`, `class`                  | —                  |
| `GroupBox` | `title`, `variant`, `class`                                  | —                  |
| `DescriptionList` | `items`, `columns`, `bordered`, `horizontal`, `class` | —                 |
| `StatusBar` | `left`, `right`, `class`                                    | —                  |
| `Toolbar`  | `class`                                                      | —                  |
| `Resizable` | `axis`, `sizes`, `class`                                    | —                  |
| `Calendar` | `value`, `months`, `weekStartsOn`, `disabled`, `controlled`, `class` | `change`    |
| `Tree`     | `items`, `value`, `controlled`, `class`                      | `change`, `expand`, `collapse` |
| `Diff`     | `patch`, `mode` (`unified`, `split`), `lineNumbers`, `softWrap`, `header`, `class` | `change` |
| `Carousel` | `value`, `axis` (`horizontal`, `vertical`), `looping`, `controls`, `pagination`, `controlled`, `class` | `change` |
| `Sidebar`  | `items`, `value`, `collapsed`, `collapsible` (`icon`, `offcanvas`, `none`), `side` (`left`, `right`), `controlled`, `class` | `change` |
| `Chart`    | `chart` (`line`, `bar`, `pie`), `data`, `color`, `name`, `curve` (`linear`, `natural`, `step`), `dot`, `horizontal`, `xAxis`, `xTicks`, `yAxis`, `yTicks`, `grid`, `yMin`, `yMax`, `innerRadius`, `padAngle`, `interactive`, `class` | — |
| `Canvas`   | `commands`, `class` | `pointerDown` |
| `Command`  | `items`, `placeholder`, `class` | `confirm` |
| `Settings` | `pages`, `class` | `change` |
| `Toggle`   | `checked`, `label`, `icon`, `variant` (`ghost`, `outline`), `tooltip`, `disabled`, `controlled`, `class` | `change` |
| `DropdownButton` | `label`, `icon`, `variant` (`primary`, `secondary`, `ghost`, `danger`), `outline`, `items`, `menuLabel`, `tooltip`, `disabled`, `class` | `click`, `menu` |
| `Label`    | `content`, `secondary`, `masked`, `highlight`, `class`       | —                  |
| `Empty`    | `icon`, `title`, `description`, `class`                      | —                  |
| `Shimmer`  | `content`, `duration`, `spread`, `reverse`, `once`, `class`  | —                  |
| `InputGroup` | `value`, `placeholder`, `prefix`, `suffix`, `prefixIcon`, `suffixIcon`, `disabled`, `readonly`, `invalid`, `controlled`, `class` | `change`, `submit` |
| `Clipboard` | `value`, `tooltip`, `class`                                 | `copied`           |
| `AlertDialog` | `open`, `title`, `description`, `icon`, `okText`, `cancelText`, `class` | `ok`, `cancel` |
| `Sheet`    | `open`, `side` (`left`, `right`, `top`, `bottom`), `title`, `size`, `class` | `close` |
| `Popover`  | `open`, `anchor` (eight corners), `arrow`, `class`           | `change`           |
| `HoverCard` | `openDelay`, `closeDelay`, `anchor` (eight corners), `class` | `change`           |
| `DatePicker` | `value` (`YYYY-MM-DD`, or `YYYY-MM-DD/YYYY-MM-DD` with `range`), `range`, `placeholder`, `disabled`, `controlled`, `class` | `change` |
| `TimeField` | `value` (`HH:MM`, or `HH:MM:SS` with `seconds`), `seconds`, `disabled`, `controlled`, `class` | `change` |
| `ColorPicker` | `value` (`#RRGGBB` or `#RRGGBBAA`), `palette`, `controlled`, `class` | `change` |
| `Combobox` | `options`, `value` (a list of option values), `placeholder`, `searchable`, `disabled`, `controlled`, `class` | `change` |
| `Bubble`   | `variant` (`filled`, `secondary`, `muted`, `tinted`, `outline`, `ghost`, `destructive`), `alignment` (`start`, `end`), `class` | — |
| `Message`  | `alignment` (`start`, `end`), `avatarName`, `avatarSrc`, `class` | —                  |
| `MessageHeader` | `class`                                                 | —                  |
| `MessageFooter` | `class`                                                 | —                  |
| `Marker`   | `content`, `icon`, `variant` (`plain`, `separator`, `border`), `alignment` (`start`, `center`, `end`), `loading`, `loadingStyle` (`spinner`, `shimmer`), `class` | — |
| `Attachment` | `status` (`pending`, `uploading`, `processing`, `failed`, `complete`), `progress`, `title`, `description`, `tooltip`, `axis` (`horizontal`, `vertical`), `src`, `data`, `class` | `click`, `remove`, `retry` |
| `MessageScroller` | `scrollbar`, `jumpButton`, `jumpLabel`, `class`             | —                  |
| `Questionnaire` | `items`, `class`                                              | `change`, `submit` |
| `List`          | `value`, `controlled`, `searchable`, `placeholder`, `hasMore`, `loading`, `loadMoreThreshold`, `class` | `change`, `confirm`, `query`, `loadMore` |
| `ListItem`      | `value` (required), `disabled`                                 | —                  |

Every type also accepts `a11y` (section 16). The components from gpui-kit's Basic, Form, Layout
and Advanced groups are described in section 17; the data components (`Tree`, `Diff`,
`Carousel`) in section 18, `Sidebar` and `Chart` in section 19, the simple components of
section 23 (`Toggle`, `DropdownButton`, `Label`, `Empty`, `Shimmer`, `InputGroup`,
`Clipboard`), the overlays of section 24 (`AlertDialog`, `Sheet`, `Popover`, `HoverCard`)
the form controls with state of section 25 (`DatePicker`, `TimeField`, `ColorPicker`,
`Combobox`) and the chat components of section 26 (`Bubble`, `Message`, `MessageHeader`,
`MessageFooter`, `Marker`, `Attachment`, `MessageScroller`, `Questionnaire`), and `List` with
its `ListItem` rows in section 27.

- A known prop with the wrong type or value (for example `disabled: "yes"` or
  `variant: "huge"`) produces `invalid_props` and the operation is not applied. `null` is
  always allowed (it removes the prop). Unknown props are ignored, with a warning on stderr.
- `Button` without `variant` uses the gpui-component default style.
- `windowControl` makes a `div` a part of the window's title bar, handled by the system:
  `drag` moves the window (with snapping, and double-click to maximize or restore),
  `minimize`, `maximize` (maximize or restore) and `close` act as the system buttons. These
  divs do not need `click` events; `close` ends in `windowClosed` as usual. Meant for
  `frameless` windows, but it also works in a normal one. Inside a `windowControl` area,
  `Button`, `Checkbox`, `Input`, a `div` with `click` and a nested `windowControl` div take
  their own clicks, so a title bar can hold a search box or buttons; everything else in it
  (text, icons, plain divs) is part of the area.
- `Icon` draws one of gpui-component's bundled icons (Lucide). It takes the current text
  size and color unless `class` sets them (`size-4`, `text-muted-foreground`). `name` is
  one of: `a-large-small`, `arrow-down`, `arrow-left`, `arrow-right`, `arrow-up`, `asterisk`, `ban`, `battery-charging`, `battery-full`, `battery-low`, `battery-medium`, `battery-warning`, `battery`, `bell`, `book-open`, `bot`, `building-2`, `calendar`, `case-sensitive`, `chart-pie`, `check`, `chevron-down`, `chevron-left`, `chevron-right`, `chevron-up`, `chevrons-up-down`, `circle-alert`, `circle-check`, `circle-user`, `circle-x`, `close`, `copy`, `cpu`, `dash`, `delete`, `ellipsis-vertical`, `ellipsis`, `external-link`, `eye-off`, `eye`, `file-text`, `file`, `folder-closed`, `folder-open`, `folder`, `frame`, `gallery-vertical-end`, `github`, `globe`, `hard-drive`, `heart-off`, `heart`, `inbox`, `info`, `inspector`, `layout-dashboard`, `loader-circle`, `loader`, `map`, `maximize`, `memory-stick`, `menu`, `mic`, `minimize`, `minus`, `moon`, `network`, `palette`, `panel-bottom-open`, `panel-bottom`, `panel-left-close`, `panel-left-open`, `panel-left`, `panel-right-close`, `panel-right-open`, `panel-right`, `pause`, `play`, `plus`, `redo-2`, `redo`, `refresh-cw`, `replace`, `resize-corner`, `rotate-cw`, `search`, `settings-2`, `settings`, `sort-ascending`, `sort-descending`, `square-terminal`, `square`, `star-fill`, `star-off`, `star`, `sun`, `thumbs-down`, `thumbs-up`, `triangle-alert`, `undo-2`, `undo`, `user`, `window-close`, `window-maximize`, `window-minimize`, `window-restore`. Any other name is `invalid_props`.
- `contextMenu` and `MenuBar` are described in section 10; `Dialog` and `tooltip` in section 14;
  `AlertDialog`, `Sheet`, `Popover` and `HoverCard` in section 24; `VirtualList`, `Table` and
  `Tabs` in section 15.
- **Form controls.** `Switch` works like `Checkbox` (`change` carries `checked`).
  `RadioGroup` and `Select` take `options`, a list of `{"value": "...", "label": "...",
  "disabled": true?}` that `setProps` replaces whole; `value` is the selected option's
  `value`, and one that is not an option selects nothing (`Select` then shows its
  `placeholder`). Choosing another option sends `change` with its `value`; choosing the
  selected one again sends nothing. `searchable` adds a search box to the `Select` list.
  `Slider` takes numbers: `min` (default 0), `max` (default 100, it must be above `min`),
  `step` (above 0, default 1) and `value` (default `min`, kept within the range); it sends
  `change` with the number continuously while dragged. `Textarea` is a multi-line `Input`:
  Enter adds a line, so it has no `submit`, and it is always synced (no `controlled`).
- `Switch`, `RadioGroup`, `Select`, `Slider`, `Tabs` and `Table` (its `selected`) support
  both modes below, like `Checkbox`.
- `text` keeps spaces and line breaks, but tabs are not drawn: replace them with spaces.
- `Image` draws a local image file: PNG, JPG, GIF (animated), WebP, BMP, ICO, TIFF, AVIF,
  SVG and the other formats GPUI decodes. `src` is a file path; a relative path is resolved
  against the host's working directory (by default the client's), so prefer absolute paths.
  URLs are not supported. `fit` works like CSS `object-fit` (default `contain`); size the
  node with classes (`w-32 h-32`, `size-full`). To make it clickable, wrap it in a `div`.
- `data` draws bytes the client already has, as standard base64 (RFC 4648, padding included,
  no `data:` prefix). It is meant for remote images: the client downloads the file and sends
  its bytes; the host opens no network connections. While `data` is non-empty it wins over
  `src`. The host decodes it once and keeps it in a file of its own per node (in the system
  temp folder), removed when the node is removed or `data` is cleared with `null` or `""`
  (the node then goes back to `src`). A value that is not base64 is `invalid_props`.
- Each time the client sends `src` or `data` (in `create` or `setProps`, even with the same
  path or bytes), the host reads the file again, so changed content shows up. Otherwise the
  image stays cached. Prefer `src` when the file is already on the host's disk: `data` stays
  in the retained tree, so it also costs memory (about 4/3 of the bytes, as in the JSON).
- A missing or unreadable file is not an error: the node stays empty and the host warns on
  stderr. An SVG is rasterized at its own size, so it looks blurry when drawn much larger;
  use `Icon` or a PNG of the right size for that.
- `Input` (text) and `Checkbox` (`checked`) state works in one of two modes, chosen per node
  with `controlled`:
  - **Synced** (default): the state lives in the host and changes as soon as the user
    interacts. The host reports it with `change` (if subscribed) and a `setProps` can impose
    another value.
  - **Controlled** (`controlled: true`): the host does not change the state on its own. It
    emits `change` with the requested value and shows it only once the client confirms it
    with `setProps` (`checked` or `value`). The client may transform the value (for example,
    upper-case it) or reject it by not answering. In a controlled `Input` the host reverts
    every edit to the client's last `value`; since GPUI moves the cursor to the end on every
    programmatic change, this mode suits short fields, not mid-text editing or IME.

## 6. Host → client messages

```json
{"type":"event","id":3,"event":"click"}
{"type":"event","id":5,"event":"change","value":"current text"}
{"type":"event","id":7,"event":"change","checked":true}
{"type":"event","id":5,"event":"submit","value":"current text"}
{"type":"event","id":9,"event":"menu","value":"copy"}
{"type":"event","id":11,"event":"change","value":7.5}
{"type":"event","id":15,"event":"copied","value":"https://example.com"}
{"type":"event","id":13,"event":"sort","value":{"column":"name","direction":"asc"}}
{"type":"windowClosed","id":2}
{"type":"windowEvent","id":2,"event":"resize","x":120,"y":80,"width":400,"height":300}
{"type":"tray","event":"menu","value":"quit"}
{"type":"shortcut","keys":"ctrl-s","window":1}
{"type":"notification","id":"saved","window":1}
{"type":"result","request":7,"paths":["C:\\a.txt"]}
{"type":"error","code":"unknown_node","message":"Node 42 does not exist","op":"setProps"}
```

`value` is a string, except for `Slider` and `Carousel` (a number), `Table` (a row index,
`null` when the selection is cleared, or the `sort` object of section 15) and `Diff` (the
selected lines object of section 18, or `null` when the selection is cleared).

`windowClosed` is sent only when the user closes a window, with the window's `id` (1 for
the main window); after the last open window the host exits. A window the client closes with
`closeWindow` does not produce it. If the client closes stdin, the host exits without sending it.

`windowEvent` carries a window event the client opted into with `window.events` (section 3);
its `event` is `focus`, `blur`, `resize`, `move` or `visibility`.

`message` is human-readable and not part of the contract; match on `code` (and `op`).

Error codes:

- `invalid_json`: the line is not JSON, not an object, or a message with a known `op` but
  missing or mistyped fields (then it includes `op`). An unknown value inside a field of a
  known `op` (for example `"theme":"sepia"`) is also `invalid_json`.
- `unknown_op`: `op` is missing or not a known operation.
- `unknown_node`: the operation names a node id that does not exist.
- `unknown_type`: `create` with a type not listed in section 5.
- `invalid_props`: see section 5.
- `invalid_operation`: the operation cannot be applied to the tree: duplicate id, id 0, a
  cycle, or a `before` that is not a child of `parent`.
- `protocol_mismatch`: see section 2.

Unknown fields are ignored, so a newer client does not break an older host.

## 7. Text inputs

- **The text lives in the host.** While the user types, the host handles cursor, selection
  and IME without asking the client.
- The host emits `change` with the current value. The client does **not** need to answer.
- If the client sends `setProps` with `value`, the host replaces the content. To avoid
  overwriting what the user is typing, the client should only send `value` when it wants to
  impose one (for example, clearing the field after `submit`).
- If the imposed `value` equals the current text, the host leaves the field alone, so an
  echo of the same text does not move the cursor. Controlled mode is described in section 5.

## 8. Classes

The `class` prop takes a Tailwind-like class string. Unknown classes are ignored, with a
warning on stderr (once per class).

- **Layout:** `flex`, `flex-row`, `flex-col`, `flex-1`, `flex-wrap`, `items-start|center|end`, `justify-start|center|end|between`
- **Spacing:** `gap-{n}`, `p-{n}`, `px-{n}`, `py-{n}`, `pt|pr|pb|pl-{n}`, `m-{n}`, `mx-{n}`, `my-{n}` (GPUI's spacing scale)
- **Size:** `w-full`, `h-full`, `size-full`, `w-{n}`, `h-{n}`, `size-{n}`, `min-w-0`, `min-h-0`, `shrink-0`, `flex-none`
- **Overflow:** `overflow-hidden`; on `div` only, `overflow-scroll`, `overflow-x-scroll` and
  `overflow-y-scroll` make it a scroll area with the theme's scrollbar, which keeps its
  position across commits. A `flex-1` child that should scroll usually needs `min-h-0`
  (or `min-w-0`) on itself or its parent, as in CSS.
- **Borders:** `border`, `border-t|r|b|l` (1px), `rounded-sm|md|lg|full`
- **Text:** `text-xs|sm|base|lg|xl`, `font-medium`, `font-bold`, `font-mono` (the theme's
  monospace font), `truncate`, `whitespace-normal`, `whitespace-nowrap`, `whitespace-pre`
  (the same as `nowrap`: spaces and line breaks are always kept)
- **Interaction:** `cursor-pointer`; on `div` only, `hover:` followed by a theme color class
  (`hover:bg-accent`, `hover:text-primary`, `hover:border-ring`)
- **Opacity:** `opacity-{n}`, with `n` from 0 to 100 in steps of 5. On a `div` it fades its
  children too.
- **Transitions:** `transition` makes a node's background, text and border colors and its
  opacity change smoothly instead of at once, whatever changes them: new classes from the
  client, a theme switch, or `hover:` on a `div`. `duration-{75|100|150|200|300|500|700|1000}`
  (milliseconds, default 150) and `ease-linear|in|out|in-out` (default `in-out`) tune it. A
  change during a transition starts from the value on screen. Sizes, spacing and position do
  not animate (they change the layout). An unset text or border color has no known start
  value, so changing from or to it is immediate; an unset background fades as transparent.
- **Animations** (loop while the class is present): `animate-spin` turns an `Icon` once per
  second (it is ignored on other node types, which cannot rotate); `animate-pulse` takes any
  node's opacity from 1 to 0.5 and back every 2 seconds. The host only redraws while
  something moves, at most 60 times a second, and a node that loops is drawn by a pass of its
  own, so it does not rebuild the nodes around it on every frame.
- **Theme colors** (gpui-component theme colors, not the Tailwind palette): `bg-{color}`,
  `text-{color}` and `border-{color}`, where `{color}` is one of `background`, `foreground`,
  `primary`, `primary-foreground`, `secondary`, `secondary-foreground`, `muted`,
  `muted-foreground`, `accent`, `accent-foreground`, `danger`, `danger-foreground`,
  `success`, `success-foreground`, `warning`, `warning-foreground`, `info`,
  `info-foreground`, `popover`, `popover-foreground`, `border`, `input`, `ring`. They follow
  the active theme (section 3).

## 9. Conformance fixtures

[`conformance/`](../conformance/) holds wire-level transcripts of this protocol. Each `.txt`
file is one session against `host --no-window`:

```
# comment
> {"op":"hello","protocol":1}
< {"type":"ready","protocol":1}
```

- `>` lines are sent to the host verbatim (they may be invalid JSON on purpose).
- `<` lines are the messages the host must write to stdout, in order. Only the fields
  listed are compared, so `message` is usually left out.
- After the last line the runner closes stdin. The host must then exit successfully, with
  no stdout output beyond the `<` lines.

The runner is `packages/host/tests/conformance.rs` (`cargo test --workspace` at the root).
The transcripts test the host. A client checks the other side: the lines it writes for each
operation must appear among the `>` lines (field order aside).

**`simulate` (fixtures only).** With `--no-window`, and only then, the host also accepts a
`simulate` message that plays a user action, so fixtures can cover section 6. It is not part
of the client contract: without `--no-window` it is `unknown_op`.

```json
{"op":"simulate","id":3,"event":"click"}
{"op":"simulate","id":5,"event":"change"}
{"op":"simulate","id":7,"event":"change","value":"new text"}
{"op":"simulate","id":7,"event":"submit","value":"new text"}
{"op":"simulate","event":"windowClosed","id":2}
{"op":"simulate","id":9,"event":"menu","value":"copy"}
{"op":"simulate","event":"key","value":"ctrl-s"}
{"op":"simulate","id":7,"event":"keyDown","value":"escape"}
{"op":"simulate","id":3,"event":"drop","value":["C:\\a.txt"]}
{"op":"simulate","event":"trayClick"}
{"op":"simulate","event":"trayMenu","value":"quit"}
{"op":"simulate","id":13,"event":"change","value":2}
{"op":"simulate","id":13,"event":"sort","value":{"column":"name","direction":"desc"}}
{"op":"simulate","id":15,"event":"change","value":"src/lib.rs"}
{"op":"simulate","id":15,"event":"expand","value":"src"}
{"op":"simulate","id":17,"event":"change","value":{"path":"notes.txt","side":"modified","start":1,"end":2,"endSide":"modified"}}
{"op":"simulate","id":19,"event":"change","value":2}
```

- It acts at once on the committed tree, like a user acting on the screen: pending
  operations do not count until their `commit`.
- It runs the same code as the window's handlers, so the event is only sent if the node
  subscribed to it, and nothing happens on a `disabled` Button, Checkbox or Input.
- `click` on `div` and `Button`; `change` on `Checkbox` and `Switch` (a click: it toggles
  `checked` unless controlled); `change` and `submit` on `Input`, with the text in `value`;
  `change` on `Textarea` (text), `RadioGroup` and `Select` (an option's `value`: an unknown
  option is `invalid_operation`, a disabled or already selected one does nothing) and
  `Slider` (a number within `min` and `max`, else `invalid_operation`).
- `menu` on `div` (its `contextMenu`) and `MenuBar` plays choosing the item whose `id` is
  `value`, in any submenu. A disabled item, or one inside a disabled submenu, does nothing;
  an `id` the node's menus do not have is `invalid_operation`.
- `windowClosed` plays the user closing window `id` (1 when absent): it sends
  `{"type":"windowClosed","id":...}` and forgets the window; the host exits when no window
  with a `root` is left and there is no tray.
- `key` plays a key pressed in window `id` (1 when absent) with no control using it: a
  registered shortcut or a `MenuBar` item's `shortcut` (section 12). `keyDown` on a `div` or
  `Input` and `doubleClick` on a `div` (which sends `click` first) report to that node. A key
  that does not parse is `invalid_json`.
- `drop` on a `div` plays files dropped on it; `value` is the list of paths.
- `ok` and `cancel` on an open `Dialog` play the user's answer: the event is sent and `open`
  becomes `false`. On a closed one they are `invalid_operation`.
- `change` on `Tabs` works like on `RadioGroup` (over `tabs`, or over its `Tab` children).
  On a `Table`, `change` selects
  the row index in `value` (`null` clears the selection; the selected row again does
  nothing) and `doubleClick` selects the row, then reports it. A row past the end is
  `invalid_operation`. `sort` takes the column and direction in `value` (the window cycles
  them, `simulate` names them); a column that is not `sortable` is `invalid_operation`.
- `change` on a `Tree` selects the item whose `id` is `value` (the selected item again does
  nothing, a `disabled` one does nothing, an unknown one is `invalid_operation`); `expand`
  and `collapse` report a folder's `id` (a leaf does nothing, an unknown id is
  `invalid_operation`).
  `change` on a `Diff` takes `null` or the selected lines object of section 18 (a path the
  patch does not have is `invalid_operation`, a malformed object is `invalid_json`).
  `change` on a `Carousel` takes a slide index (past the last slide is `invalid_json`).
- `trayClick` and `trayMenu` (with the item `id` in `value`) play a left click on the tray
  icon and choosing an item of its menu. Without a tray they are `invalid_operation`; a
  disabled item does nothing and an unknown one is `invalid_operation`. An unknown window is `invalid_operation`.
- Errors: `unknown_node` for an id not in the committed tree, `invalid_operation` for an
  event the node type does not have, `invalid_json` for a missing `id`, `event` or `value`.
- Not covered: the GPUI bindings themselves (a real click or Enter reaching the handler)
  and the controlled `Input` revert (section 7), which needs the window's text state. Those
  are checked by hand in a window.

## 10. Menus

```json
{"op":"create","id":9,"type":"div","props":{"contextMenu":[
  {"id":"copy","label":"Copy","icon":"copy"},
  {"separator":true},
  {"id":"wrap","label":"Word wrap","checked":true},
  {"label":"Export","items":[{"id":"png","label":"PNG"},{"id":"pdf","label":"PDF","disabled":true}]}
]},"events":["menu"]}
{"op":"create","id":2,"type":"MenuBar","props":{"menus":[
  {"label":"File","items":[{"id":"open","label":"Open"},{"id":"quit","label":"Quit"}]}
]},"events":["menu"]}
```

A menu is a list of items, each one of:

- **Item**: `id` (a string chosen by the client) and `label`; optional `icon` (a bundled icon
  name, section 5), `shortcut` (a key, section 12), `disabled` and `checked` (draws a check mark; the client toggles it by
  sending the list again).
- **Submenu**: `label` and `items` (another list), optional `icon` and `disabled`. A
  disabled submenu is drawn as a disabled item and does not open.
- **Separator**: `{"separator":true}`.

Choosing an item sends `{"type":"event","id":<node>,"event":"menu","value":"<item id>"}`
if the node subscribed to `menu`. The node already tells the client which window it was in.

- `contextMenu` (on `div`) opens at the pointer on right-click. An empty list opens nothing.
  To give another node type a context menu, wrap it in a `div`.
- `MenuBar` draws one button per entry of `menus` (`{"label":…, "items":[…]}`); clicking a
  button opens its items below it. The client places it like any node: at the top of the
  window, or inside the title bar of a `frameless` window (it takes its own clicks inside a
  `windowControl` area, like `Button`).
- Lists are validated as a whole: a malformed item (missing `label`, an item without `id`
  or `items`, a wrong type, an unknown icon) rejects the operation with `invalid_props`.
  `setProps` replaces the whole list; `null` removes it.
- Not supported: keyboard navigation between the `MenuBar`
  menus (left/right) and switching menus by hovering while one is open, and the native
  macOS menu bar.

## 11. System tray

```json
{"op":"tray","icon":"C:\\app\\icon.png","tooltip":"My app","menu":[
  {"id":"show","label":"Show"},{"id":"notify","label":"Notifications","checked":true},
  {"separator":true},{"id":"quit","label":"Quit"}]}
{"op":"removeTray"}
```

`tray` shows the app's icon in the Windows notification area, or updates it. One per app.

- Applied on `commit`. Every field is optional; an update sends only what changes.
- `icon`: an image file (PNG, ICO, JPG and the other formats `Image` reads), drawn at the
  tray's size. A missing or unreadable file leaves the slot empty, with a warning on stderr.
  The file is read again on every `tray` message that carries it.
- `tooltip`: shown when hovering the icon.
- `menu`: opens on right-click. It uses the item format of section 10, but it is a native
  Windows menu: it does not follow the app theme and item `icon`s are not drawn. A `&` in a
  label is shown as is. An item with `checked` (`true` or `false`) toggles its own mark when
  chosen, as native menus do; the client keeps it in step by sending `menu` again, which
  always rebuilds the menu.
- A malformed `menu` or a mistyped field is `invalid_json`.
- `removeTray` removes the icon on `commit`; without a tray it is `invalid_operation`.

Events (always sent, no subscription):

```json
{"type":"tray","event":"click"}
{"type":"tray","event":"menu","value":"quit"}
```

`click` is a left click on the icon; `menu` carries the chosen item's `id`.

**Lifecycle.** While there is a tray, closing the last window does not end the host: it
sends `windowClosed` and keeps running, so the app can live in the tray and open a window
again with `window`. Removing the tray with no window open ends the host; so does closing
stdin, which also removes the icon.

Not supported: several tray icons, double click, notification balloons, and hiding a window
without closing it (reopening it starts from the client's tree; an `Input`'s text that the
client does not hold is lost).

## 12. Keyboard

```json
{"op":"shortcuts","keys":["ctrl-s","ctrl-shift-p","f5"]}
{"op":"focus","id":5}
{"type":"shortcut","keys":"ctrl-s","window":1}
{"type":"event","id":7,"event":"keyDown","value":"escape"}
```

Keys use GPUI's format: modifiers (`ctrl`, `alt`, `shift`, `win`) and a key joined by `-`,
such as `ctrl-shift-s`, `alt-left`, `enter`, `escape`, `space`, `f5`. A key matches only
with exactly those modifiers.

- `shortcuts` sets the app-wide shortcuts on `commit`; each message replaces the list. A
  key pressed in any window of the app sends `shortcut` with the key as the client wrote it
  and the window. Shortcuts are checked before the focused control, so they win: registering
  `ctrl-c` stops an `Input` from copying. An invalid key rejects the message with
  `invalid_json`.
- A menu item's `shortcut` (section 10) is drawn next to its label. In a `MenuBar` it also
  works without opening the menu: the key sends the same `menu` event as a click, while the
  `MenuBar` is in the focused window and the item is enabled. In a `contextMenu` or the tray
  menu it is only shown. Registered `shortcuts` are checked first. Like them, a `MenuBar`
  shortcut wins over the focused control: an Edit › Copy item with `ctrl-c` sends `menu`
  instead of letting an `Input` copy (and if the `MenuBar` is not subscribed to `menu`, the
  key is used up with nothing sent).
- `keyDown` (opt-in) on a `div` makes it focusable: a click on it (not on a control inside
  it) or Tab gives it the focus, and while it or something inside it has the focus each key
  sends `keyDown` with the key as `value`. On an `Input` it reports the keys the Input gets
  (Escape, arrows, Enter); the Input still handles them.
- `doubleClick` (opt-in) on a `div` is sent on the second click of a double click; both
  clicks also send `click`.
- `focus` gives the keyboard focus to an `Input`, a `Textarea`, a `NumberInput`, an `OtpInput`, a
  `Calendar`, a `Tree`, a `Table`, a `Command` (its search field), a `List` or a `div` with `keyDown`,
  once the commit is applied and its window is showing. Another type is `invalid_operation`.
- Not supported: system-wide shortcuts (when the app is not focused), key up events, and
  focus or blur events.

## 13. System actions

```json
{"op":"openDialog","request":7,"files":true,"directories":false,"multiple":true,"prompt":"Open"}
{"op":"saveDialog","request":8,"directory":"C:\\Users\\me","name":"notes.txt"}
{"op":"readClipboard","request":9}
{"op":"writeClipboard","text":"copied"}
{"op":"openUrl","url":"https://example.com"}
{"op":"openPath","path":"C:\\Users\\me\\notes.txt"}
{"type":"result","request":7,"paths":["C:\\a.txt","C:\\b.txt"]}
{"type":"result","request":8,"path":null}
{"type":"result","request":9,"text":"copied"}
```

These are **actions**: they run when the host reads them, not on `commit`. The ones that
answer carry a `request` id chosen by the client (any non-negative integer) and get a
`result` with that id, possibly after other messages (a dialog answers when the user closes
it, and the app keeps running meanwhile; several can be pending).

- `openDialog`: the system's open dialog. `files` (default `true`), `directories` (default
  `false`) and `multiple` (default `false`) choose what can be picked; at least one of
  `files` and `directories` must be `true`. `prompt` is the confirm button's text. Answers
  `paths` (a list) or `null` when cancelled.
- `saveDialog`: the system's save dialog, starting in `directory` (default: the host's
  working directory) with the suggested `name`. Answers `path` or `null`.
- `readClipboard` answers `text`, or `null` when the clipboard holds no text.
  `writeClipboard` puts `text` on it.
- `openUrl` opens a URL with its system handler (the browser for `https:`); `openPath`
  opens a file or folder with its default app.
- A malformed action is `invalid_json`. If it has a `request`, the `error` carries it too,
  since that request gets no `result`.
- With `--no-window` nothing is shown or opened: dialogs and `readClipboard` answer `null`.
- `drop` (opt-in) on a `div`: files dropped on it from the system (the Explorer) send
  `{"type":"event","id":3,"event":"drop","paths":["C:\\a.txt"]}`.
- Not supported: file type filters in dialogs (GPUI has none), images on the clipboard,
  "show in folder", and dragging files out of the app.

## 14. Dialogs, notifications and tooltips

```json
{"op":"create","id":20,"type":"Dialog","props":{"open":true,"title":"Rename","okText":"Save","cancelText":"Cancel","width":400},"events":["ok","cancel"]}
{"op":"notify","window":1,"id":"saved","type":"success","title":"Saved","message":"notes.txt","autohide":true,"system":false}
{"type":"notification","id":"saved","window":1}
```

**Dialog.** A `Dialog` node shows a modal dialog while its `open` prop is `true`: gpui-component's
dialog, with a dimmed overlay, a title, an OK button and an optional Cancel button. Its
children are the body (any nodes: text, an `Input`…); they are drawn inside the dialog, not
where the `Dialog` sits in the tree. Put the `Dialog` anywhere in its window's tree.

- Props: `open`, `title`, `okText` (default `OK`), `cancelText` (adds a Cancel button),
  `width` (pixels, above 0) and `class` (for the body that holds the children).
- `ok` (the OK button) and `cancel` (the Cancel button, Escape, the X or a click outside) are
  opt-in events. Either one closes the dialog and sets `open` to `false` in the host, like a
  synced `Checkbox`: send `open: true` again to reopen it. Closing it with `open: false`
  sends nothing.
- One dialog at a time per window: if several are open, the one with the lowest id shows.
- Changes to its props and to the body show while it is open.

**notify** is an action (section 13): it shows a toast in a window now, not on `commit`.

- `message` (required), `title`, `type` (`info` by default, `success`, `warning`, `error`),
  `window` (default 1; a window that is not open is `invalid_operation`), `autohide`
  (default `true`).
- `id`: a click on the toast sends `{"type":"notification","id":...,"window":...}`, and a new
  toast with the same `id` replaces the previous one.
- `system: true` also posts it to the Windows notification center, where it shows under the
  app name `gpui-bridge` (the host registers that identity; apps cannot name it yet).
- With `--no-window` it is validated and nothing is shown.

**tooltip** (a string prop on `div` and `Button`) is shown when the pointer rests on the node.
For another type, wrap it in a `div`.

Not supported: several dialogs stacked, dialogs without the OK button or with more buttons,
notification actions (buttons inside a toast), and rich tooltips.

## 15. Lists, tables and tabs

```json
{"op":"create","id":30,"type":"VirtualList","props":{"rowHeight":28,"class":"flex-1"}}
{"op":"create","id":31,"type":"Table","props":{"columns":[{"key":"name","name":"Name","width":240,"sortable":true},{"key":"size","name":"Size","align":"right"}],"rows":[{"name":"a.txt","size":"12 KB"}],"selected":0,"stripe":true},"events":["change","doubleClick","sort"]}
{"op":"create","id":32,"type":"Tabs","props":{"tabs":[{"value":"general","label":"General"},{"value":"advanced","label":"Advanced"}],"value":"general","variant":"underline"},"events":["change"]}
{"type":"event","id":31,"event":"change","value":3}
{"type":"event","id":31,"event":"sort","value":{"column":"name","direction":"desc"}}
```

**VirtualList** shows its children as rows of `rowHeight` pixels (above 0, default 32) and
builds only the rows in view, so a list of thousands of rows scrolls as fast as a short one.
Each child is one row: any node, with its own events, classes and `hover:`. Rows fill the
list's width and their height. The list fills its container and scrolls with the theme's
scrollbar, so give it a height (`flex-1`, `h-64`). The scroll position is kept across
commits; removing the list from the window (or moving it out) resets it.

- The client still sends every row: thousands of rows mean thousands of `create`
  operations once. For big plain data, a `Table` sends one prop instead.
- Rows do not keep a running `transition` or the host's `hover:` animation when they scroll
  out of view and back.

**Table** draws gpui-component's data table over two props:

- `columns`: a list of `{"key", "name", "width"?, "align"?, "sortable"?}`. `width` is in
  pixels (default 100; the user can resize columns), `align` is `left` (default), `center`
  or `right`, and `sortable: true` shows a sort button in the header.
- `rows`: a list of objects, each with a cell per column `key`. Cells are text: strings,
  numbers and booleans are shown as written, `null` or a missing key is empty; lists and
  objects are `invalid_props`. `setProps` replaces the whole list.
- `selected` is the index of the selected row in `rows` (a whole number; one past the end
  selects nothing). The user selects with a click or the arrow keys, which sends `change`
  with the index; Escape clears it and sends `change` with `null`. `selected` follows the
  synced and controlled modes of section 5 (`controlled`).
- `doubleClick` (opt-in) sends the row's index; the first click already selected it, so
  `change` comes first.
- `sort` (opt-in) sends `{"column": key, "direction": "asc" | "desc" | "none"}` when the user
  clicks a sort button; each click goes none → desc → asc → none. The host does not sort:
  the app sends `rows` again in the new order (and usually a new `selected`).
- `stripe` shades every other row; `bordered` (default `true`) draws the outer border.
- Only the rows in view are drawn. A Table can take the focus (`focus`, section 12) so the
  arrow keys move the selection.
- The sort arrows and the column widths the user set live in the window: a Table removed
  from the window (or moved out) shows them reset when it comes back, while `rows` keeps
  the app's order.

**Tabs** is a tab bar: `tabs` is a list like `RadioGroup`'s `options` (`value`, `label`,
`disabled`), `value` is the selected tab's `value`, and `variant` is `tab` (default),
`outline`, `pill`, `segmented` or `underline`. Choosing another tab sends `change` with its
`value`; it follows the synced and controlled modes of section 5. Only the bar is drawn:
the app shows the selected tab's content itself, for example by moving the matching panel
into the window.

A `Tabs` may instead take **`Tab` children**, one per tab, which then replace `tabs`
altogether (a `Tabs` with `Tab` children ignores `tabs`; with none it works as before):

```json
{"op":"create","id":40,"type":"Tabs","props":{"value":"files","variant":"underline"},"events":["change"]}
{"op":"create","id":41,"type":"Tab","props":{"value":"files","a11y":{"label":"Files"}}}
{"op":"create","id":42,"type":"Icon","props":{"name":"folder","class":"size-4"}}
{"op":"create","id":43,"type":"text","props":{"content":"Files"}}
{"op":"create","id":44,"type":"Tag","props":{"label":"12"}}
{"op":"append","parent":41,"child":42}
{"op":"append","parent":41,"child":43}
{"op":"append","parent":41,"child":44}
{"op":"append","parent":40,"child":41}
```

A `Tab` is `value` —required and not empty, it is what `change` reports—, `label` (its text)
and `disabled`. Its children are drawn inside the chip, after the `label`, in a row with a gap,
so a tab can be **made of nodes**: an `Icon`, a `text`, a `Tag` with a count and so on. The row
has the chip's height and cuts what is taller, so a count goes beside the text (a `Badge` around
a small icon would cover it). A `Button`, `Checkbox` or `Switch` that is the **last** child goes
after the row instead, at its own size (a close button). Its `class` styles the chip itself, so
`flex-1` makes the tabs share the bar's width. The `Tabs` still sends `change`, and a tab that is
`disabled` or already selected does nothing. A control inside a `Tab` (`Button`, `Checkbox`,
`Input`, `Select`, `Slider`, a `div` with `click`…) keeps its own click: it does not choose the
tab.

The bar is a `tablist` and every chip a `tab` already. The host wraps the bar in one Tab stop that
reads as the chosen tab and moves with the arrows (section 16); the `a11y.label` of a `Tabs`
becomes that stop's description. The nodes of a chip
are not the chip itself: the tab is announced as a tab, with the `a11y.label` of the `Tab` node,
or with its `label` when there is none. Three cases are the client's own
mistake and the host only draws around them: a `Tab` outside a `Tabs` is not drawn, a child of a
`Tabs` that is not a `Tab` is not a tab, and with a `value` repeated among siblings the first
one wins.

Not supported: cells with anything but text, cell or column selection, moving columns, a
context menu per row, loading more rows at the end, rows of different heights, and tabs that
can be closed or reordered.

## 16. Accessibility

GPUI reports to assistive technology (AccessKit) only the elements that have **both** an id and
a role; a focused element without them makes GPUI log a warning. Every node accepts an `a11y`
prop with those two and the ARIA-like state around them:

```json
{
  "op": "create", "id": 4, "type": "div",
  "props": {
    "class": "size-8 rounded-full bg-accent",
    "a11y": { "id": "cart", "role": "button", "label": "Cart", "keyshortcuts": "ctrl-k" }
  },
  "events": ["click"]
}
```

| key | type | meaning |
|---|---|---|
| `id` | string | A stable identifier, visible outside the process (UIA `AutomationId` on Windows). |
| `role` | string | One of the roles below. |
| `label` | string | The name a screen reader reads. |
| `description` | string | Supplementary text, read after the label. |
| `keyshortcuts` | string | The keys that activate the element (`ctrl-s`); it does not create a key binding. |
| `checked` | boolean or `"mixed"` | A checkbox's or switch's state. |
| `selected`, `expanded`, `disabled` | boolean | The state of a tab, a section, a control. |
| `value` | string | The value of a slider, a spin button, a text field… |
| `min`, `max`, `step` | number | The range of a numeric value. With any of them, a numeric `value` is reported as a number. |
| `level` | whole number from 1 | A heading's level. |
| `orientation` | `horizontal` or `vertical` | A slider's or a scroll area's direction. |

Roles (ARIA's names): `alert`, `alertdialog`, `article`, `banner`, `button`, `cell`,
`checkbox`, `columnheader`, `combobox`, `complementary`, `contentinfo`, `definition`, `dialog`,
`document`, `form`, `grid`, `gridcell`, `group`, `heading`, `img`, `label`, `link`, `list`,
`listbox`, `listitem`, `log`, `main`, `menu`, `menubar`, `menuitem`, `navigation`, `note`,
`paragraph`, `progressbar`, `radio`, `radiogroup`, `region`, `row`, `rowheader`, `scrollbar`,
`searchbox`, `separator`, `slider`, `spinbutton`, `status`, `switch`, `tab`, `table`, `tablist`,
`tabpanel`, `textbox`, `time`, `toolbar`, `tooltip`, `tree`, `treeitem`, `window`.

- The object is checked like any other prop: an unknown key, an unknown role or a wrong type is
  `invalid_props` (so a typo does not go silent). `null` removes it.
- A `div` and a `text` get the accessibility node on their own element. GPUI does not pass a
  `text`'s words to the reader, so a `text` with a `role` and no `label` is named by its
  `content`; with the `label` role it reads like text on a page (Narrator's scan mode reads it,
  Tab does not stop on it), which is how a conversation's messages are read. A component (`Button`,
  `Slider`…) is wrapped in one box that carries it, so the wrap only happens when `a11y` is
  set; the wrapper is a row and does not change the sizes inside.
- A window's root is reported as a `window` with its title, so a window whose own focus has no
  control still has an accessibility node.
- A `Button` is already reported as a button with its label; the other components name
  themselves as best they can (a `Checkbox`'s label, for example) and take `a11y` for the rest.
- Some of section 23's components keep the accessibility node the crate builds, so the host does
  not wrap them and their `role` is ignored: a `Toggle` names itself with its `tooltip` (give it a
  `tooltip` or an `a11y.label`, not both), a `Clipboard` with `a11y.label` or its `tooltip`, and
  an `InputGroup` with `a11y.label` — the field inside reads from its `placeholder`.
- Do not repeat the role a component already reports: an `Input` is a text field named by its
  `placeholder`, so `a11y.role: "textbox"` makes the reader find it twice. To name a component
  that has no label of its own (a `Select`, a `RadioGroup`, a `Progress`), give `a11y` the
  `group` role and a `label`.
- The host gives `Slider`, `Rating`, `Tabs` and `Pagination` a Tab stop (none when `disabled`)
  and keys: ←/↓ and →/↑ move one `step` (one star, one page, the previous or next enabled tab),
  Home and End go to the first and last. A key sends the same `change` a click would, in both
  synced and controlled modes (a controlled `Slider` that rounds the value should have the
  `step` it rounds to, or an arrow proposes a value the app takes back). The stop reports its own role and value — `slider` (Slider,
  Rating), `spinbutton` (Pagination) or the chosen `tab` (Tabs) — so their `a11y` only gives the
  `id`, `label`, `description` and `keyshortcuts`; its `role` and value are ignored. A `Tabs`
  reads as its chosen tab, and its `a11y.label` becomes the description.
  Without a stop (`disabled`) they read the same way. gpui-kit already reports a `Slider` as a
  slider, so its `a11y.label` names that one too.
- The other components keep gpui-component's own keyboard behaviour: `Accordion`, `Calendar` and
  `Toggle` take no Tab stop, so a keyboard-only user cannot reach them (a screen reader's own
  navigation still reads their `a11y`).

## 17. Basic, Form, Layout and Advanced components

The components of gpui-kit's index that fit the tree model without extra dependencies are
mapped onto nodes. `Editor` (syntax highlighting needs `tree-sitter`) and `Speech` (dictation
needs the system recognizer) are left out on purpose: they would add megabytes to the host
binary. `Dock` and `List` keep their own delegates and state machines
and are not mapped yet; `Tree`, `Diff` and `Carousel` are in section 18, `Sidebar` and
`Chart` in section 19, `Command` in section 21, `Settings` in section 22 and the simple
components (`Toggle`, `DropdownButton`, `Label`, `Empty`, `Shimmer`, `InputGroup`,
`Clipboard`) in section 23.

- **Alert** — an inline message: `variant` is `default` (default), `info`, `success`, `warning`
  or `error`; `banner` makes it a full-width band; `closable` shows a close button that sends
  `close`. The app removes the node, or clears `closable`.
- **Badge** — `count` (hidden at 0) up to `max` (default 99), or `dot`, or a bundled `icon`; its
  children are shown under the badge, with the count drawn over them, so a `Badge` without
  children has nothing to sit on.
- **Tag** — a label with a `variant` (`primary`, `secondary` (default), `danger`, `success`,
  `warning`, `info`), `outline` and an optional `icon`.
- **Kbd** — `keys` as one keystroke (`ctrl-s`, `f5`, `shift-enter`); `outline`. A string that is
  not a keystroke draws nothing.
- **Progress** — `value` from 0 to 100 (clamped) and `loading`.
- **Spinner**, **Skeleton** — size them with `class` (`size-4`, `h-4 w-32`);
  `Skeleton.secondary` uses the muted color.
- **Rating** — stars: `value`, `max` (5), `disabled`. A click sends `change` with the count; it
  follows the synced and controlled modes of section 5.
- **Pagination** — `page` (1-based) of `total` (default 1), `compact`. A click sends `change`
  with the page, kept in `page` in synced mode.
- **Collapsible** — shows its children while `open`; the trigger is the app's (a `Button` that
  sets `open`), so it has no event of its own.
- **Accordion** — sections as `items`: `{"id", "title", "content", "open", "disabled"}`; the
  content is **text** (a section that needs rich content uses a `Collapsible` with children).
  `multiple` (default `false`), `bordered` (default `true`), `disabled`. A toggle sends `change`
  with the section's `id` and, in synced mode, rewrites that item's `open`.
- **Avatar** — a picture from `src` (an image file), or the initial of `name`.
- **Stepper** — `items` as `RadioGroup`'s `options` (`value`, `label`, `disabled`), `value` is
  the current step, `vertical` stacks them. A click sends `change` with the step's `value`.
- **NumberInput** — a text field with increment and decrement buttons (they step by 1); `value`
  is text, like an `Input`, and `change`/`submit` work the same way. Bounds are the app's job.
- **OtpInput** — `length` cells (default 6), `value`, `masked`; `change` sends the code as typed.
  A different `length` rebuilds the state (the value is kept).
- **Form** and **Field** — a `Form` lays out its `Field` children (vertically by default,
  `direction: "horizontal"`); a `Field` is a `label`, a `description`, `required` (adds the
  asterisk) and its children as the control. A child that is not a `Field` gets one without a
  label.
- **GroupBox** — a titled box (`title`) with `variant` `normal` (default), `fill` or `outline`;
  its children are the content.
- **DescriptionList** — key-value rows as `items` (`{"label", "value", "span"}`), `columns`
  (default 1), `bordered`, `horizontal` (label and value side by side).
- **StatusBar** — `left` and `right` text.
- **Toolbar** — a strip for tools; its children are its content.
- **Resizable** — panels the user can drag: `axis` (`horizontal` by default, or `vertical`) and
  `sizes`, the initial width (or height) in pixels of each panel in order; a panel without a
  size shares what is left. Append one child per panel. Dragging is not reported: the sizes
  live in the window, like a Table's column widths.
- **Calendar** — a month view: `value` (`YYYY-MM-DD`), `months` (1 to 3), `weekStartsOn`
  (`sunday` by default, or `monday`). Choosing a day sends `change` with the day (`YYYY-MM-DD`),
  kept in `value` in synced mode. A `value` that is not a date shows no day selected.

The transcripts in `conformance/basic.txt`, `conformance/layout.txt` and
`conformance/a11y.txt` are the reference for their props and events.

## 18. Trees, diffs and carousels

These three describe data in props instead of plain values. They keep one state per node in
the host and rebuild it only when the data changes, so scroll position, selection and
expansion are not lost on every commit.

### Tree

```json
{"op":"create","id":15,"type":"Tree","props":{"items":[
  {"id":"src","label":"src","expanded":true,"children":[
    {"id":"src/lib.rs","label":"lib.rs"},
    {"id":"src/ui","label":"ui","children":[{"id":"src/ui/tree.rs","label":"tree.rs"}]}
  ]},
  {"id":"README.md","label":"README.md"},
  {"id":"target","label":"target","disabled":true}
],"value":"src"},"events":["change","expand","collapse"]}
```

- `items` is a list of `{"id", "label"}` with optional `children` (the same shape, any depth),
  `expanded` (open when the tree is first built; default false) and `disabled`. `id` is a
  string chosen by the client and must be unique across the whole tree; a repeated one, a
  missing `id`/`label`, or `children` that is not a list is `invalid_props`.
- `value` is the selected item's `id`; one that is not in `items` (or a collapsed one)
  selects nothing, and a value that points inside a collapsed folder expands its ancestors.
  Selecting an item sends `change` with its `id`; in synced mode the host keeps it in
  `value`, in controlled mode the host shows the client's `value` again until it is confirmed.
- Clicking a folder selects it and toggles it, sending `expand` or `collapse` with its `id`
  (opt-in, like every event). The expansion is kept in the host while the node lives and is
  reapplied when `items` is rebuilt, except for items the client changes explicitly with
  `expanded`.
- The tree scrolls itself and virtualizes its rows: give it a bounded height (`h-64`,
  `flex-1`) with `class`.
- `focus` works on a Tree: the arrow keys move the selection, Enter toggles a folder.

### Diff

```json
{"op":"create","id":17,"type":"Diff","props":{
  "patch":"diff --git a/notes.txt b/notes.txt\n--- a/notes.txt\n+++ b/notes.txt\n@@ -1,3 +1,4 @@\n one\n-two\n+2\n three\n+four\n",
  "mode":"split","lineNumbers":true,"softWrap":true,"header":true
},"events":["change"]}
```

- `patch` is a unified or Git diff, parsed by the host; a malformed patch is `invalid_props`.
  An empty patch draws nothing. The state is rebuilt only when the patch (or `mode`) changes.
- `mode` is `unified` (default) or `split`; `lineNumbers`, `softWrap` and `header` default to
  `true`, `false` and `true`.
- Selecting source lines sends `change` with
  `{"path", "side", "start", "end", "endSide"}` (`side` and `endSide` are `original` or
  `modified`, the lines are 1-based), or `null` when the selection is cleared. The selection
  is the component's: it is not a prop.
- Like the tree, the diff scrolls itself: give it a bounded height in `class`.

### Carousel

```json
{"op":"create","id":19,"type":"Carousel","props":{"value":0,"axis":"horizontal",
  "looping":false,"controls":true,"pagination":true},"events":["change"]}
{"op":"create","id":20,"type":"text","props":{"content":"One"}}
{"op":"create","id":21,"type":"text","props":{"content":"Two"}}
{"op":"append","parent":19,"child":20}
{"op":"append","parent":19,"child":21}
```

- Each child is one slide; the host counts the node's children. Adding or removing a child
  changes the slide count and clamps the selection.
- `value` is the selected slide index (0-based); one past the last slide selects the last one.
  `axis` is `horizontal` (default) or `vertical`; `looping` wraps at the ends (default false);
  `controls` shows the previous/next buttons (default true); `pagination` shows one dot per
  slide (default false).
- Moving with the buttons, dots, the pointer or the arrow keys sends `change` with the new
  index; in synced mode the host keeps it in `value`, in controlled mode it goes back to the
  client's index until it is confirmed.

The transcript in `conformance/advanced.txt` is the reference for their props and events.

## 19. Sidebar and charts

Two more components whose content is data rather than children: a navigation sidebar and a
chart. Neither adds dependencies and both are props-only for the client.

### Sidebar

```json
{"op":"create","id":30,"type":"Sidebar","props":{
  "items":[
    {"label":"General","items":[
      {"id":"home","label":"Home","icon":"layout-dashboard"},
      {"id":"inbox","label":"Inbox","icon":"inbox"},
      {"id":"soon","label":"Soon","disabled":true}
    ]},
    {"items":[{"id":"settings","label":"Settings","open":true,
               "children":[{"id":"theme","label":"Theme"}]}]}
  ],
  "value":"home","collapsed":false,"collapsible":"icon","side":"left"
},"events":["change"]}
```

- `items` is a list of sections. A section is `{"label", "items"}`; without `label` its entries
  have no heading. Each entry is `{"id", "label"}` with optional `icon` (a bundled icon name),
  `disabled`, `open` (its submenu starts open, default false) and `children` (a submenu of
  entries, the same shape, any depth).
- `id` is a string chosen by the client and must be unique across the whole sidebar; a
  repeated one, an entry without `id` or `label`, a section without `items`, an icon that is
  not bundled, or an unknown field type is `invalid_props`.
- `value` is the active entry's `id`; one that is not in `items` leaves none active. Clicking
  an entry sends `change` with its `id`; in synced mode the host keeps it in `value`, in
  controlled mode the host shows the client's `value` again until it is confirmed. A disabled
  entry, or the active one again, sends nothing. A parent entry reports its own `id` and
  toggles its submenu.
- `collapsed` folds the sidebar to icon width (48 px); `collapsible` says what folding does:
  `icon` (default), `offcanvas` (it leaves the layout) or `none` (it ignores `collapsed`). The
  toggle is the app's: a `Button` that sets `collapsed`.
- `side` is `left` (default) or `right`. The sidebar fills the height of its box and is 255 px
  wide (`w-64` in `class` changes it); give it a bounded parent (`h-full` in a `flex` row).
- The node takes **no children**: its content is `items`. A header or a footer is the app's,
  composed next to it.

### Chart

```json
{"op":"create","id":31,"type":"Chart","props":{
  "chart":"line","name":"Visits","color":"primary",
  "data":[{"label":"Mon","value":12},{"label":"Tue","value":18},{"label":"Wed","value":9}],
  "curve":"natural","dot":true,"yAxis":true,"grid":true
}}
```

- `chart` is `line` (default), `bar` or `pie`. `data` is a list of `{"label", "value"}`: the
  category of a point or a bar, or the name of a slice. A point without a `label` string or
  with a value that is not a finite number is `invalid_props`. A thousand points are fine: the
  host parses `data` once per change and keeps it while the node lives.
- `color` names a theme color, so light, dark and custom themes come for free: `chart-1`
  (default) to `chart-5`, `primary`, `secondary`, `muted`, `accent`, `danger`, `success`,
  `warning` or `info`. A line uses it as its stroke and a bar chart as its fill; a pie cycles
  `chart-1` to `chart-5` slice by slice. `name` is the series name the hover tooltip shows (a
  pie's tooltip names the slice instead).
- A line: `curve` is `linear` (default), `natural` or `step`; `dot` puts a dot on each point;
  `yMin` and `yMax` (both, with `yMin < yMax`) pin the value axis instead of fitting the line
  from zero.
- A bar chart: `horizontal` grows the bars to the right from a left baseline (default false,
  they grow up from the bottom).
- `xAxis` (default true) shows the category axis and `yAxis` (default false) the value axis;
  `xTicks` is how many category labels a line or bar chart draws, spread evenly from the first
  point to the last (default 12; 0 draws none, and one at or above the number of points labels
  every point); `yTicks` is how many ticks the value axis carries, both ends included, which
  places its labels and the horizontal grid lines (default 5; anything below 2 counts as 2);
  `grid` (default true) draws the grid lines. `interactive` (default true) turns the hover
  crosshair, the dots and the tooltip off.
- A pie: `innerRadius` is the hole in the middle in pixels (default 0, a donut from there in)
  and `padAngle` the gap between slices in **radians** (`0.03` is a thin gap; `2` would
  swallow every slice narrower than 115 degrees).
- A chart fills its box and has **no events**: nothing it does is the app's business. Give it
  a bounded size in `class` (`h-56 w-full`, `h-64 flex-1`).

The transcript in `conformance/charts.txt` is the reference for their props and events.


## 20. Canvas

A node the client draws into: it takes a list of drawing commands in `commands` and the
host paints them with GPUI's paint API (`paint_quad`, `paint_path`). It has no children;
its content is the list.

```json
{
  "op": "create", "id": 10, "type": "Canvas",
  "props": {
    "class": "h-64 w-full",
    "commands": [
      { "op": "rect", "x": 10, "y": 10, "w": 100, "h": 60, "color": "primary", "corner_radius": 8 },
      { "op": "line", "x1": 0, "y1": 0, "x2": 200, "y2": 200, "color": "danger", "width": 2 },
      { "op": "path", "d": "M10 10 L90 10 L90 90 Z", "color": "chart-2", "width": 3 }
    ]
  },
  "events": ["pointerDown"]
}
```

| `op`   | Draws            | Required fields        | Optional fields                  |
|--------|------------------|------------------------|----------------------------------|
| `rect` | Filled rectangle | `x`, `y`, `w`, `h`     | `color`, `corner_radius` (0)     |
| `line` | Straight line    | `x1`, `y1`, `x2`, `y2` | `color`, `width` (1)             |
| `path` | Stroked path     | `d`                    | `color`, `width` (1)             |

- Coordinates are logical pixels from the top-left corner of the `Canvas`. Commands paint in
  order, so later ones cover earlier ones. Whatever falls outside the canvas is clipped.
- `d` takes absolute `M x y`, `L x y` and `Z`, separated by spaces or commas (`M10,10L90 10Z`).
  It starts with `M`. Other SVG commands are not supported.
- `color` names a theme color, like a Chart's: `chart-1` (the default) to `chart-5`, `primary`,
  `secondary`, `muted`, `accent`, `danger`, `success`, `warning`, `info`. The host resolves it
  against the current theme, so light, dark and custom themes come for free.
- `class` works like on any node; give the canvas a bounded size (`h-64 w-full`).
- `setProps` replaces the whole list; `null` removes it.
- A malformed command (unknown `op`, a missing or mistyped field, a negative `w`, `h`,
  `corner_radius` or `width`, an unknown color, bad `d`) is `invalid_props` and the operation
  is not applied.

`pointerDown` (opt-in) is a left-button press on the canvas. Its `value` is the point,
local to the canvas, in the same coordinates as the commands:

```json
{"type":"event","id":10,"event":"pointerDown","value":{"x":14.0,"y":23.5}}
```

`simulate` takes the same `value`: `{"op":"simulate","id":10,"event":"pointerDown","value":{"x":14,"y":23.5}}`.

`PROTOCOL_VERSION` stays at 1: `Canvas` is a new node type, so an older host answers
`unknown_type` to it and nothing else changes.

The transcript in `conformance/canvas.txt` is the reference for its props and events.

## 21. Command

A command palette: a search field over a filtered list of entries. Like a `Sidebar`, its
content is data in props and it takes no children.

```json
{"op":"create","id":40,"type":"Command","props":{
  "placeholder":"Type a command...",
  "items":[
    {"label":"File","items":[
      {"id":"open","label":"Open","icon":"folder","keywords":["load"]},
      {"id":"save","label":"Save","checked":true},
      {"id":"soon","label":"Soon","disabled":true}
    ]},
    {"items":[{"id":"quit","label":"Quit"}]}
  ]
},"events":["confirm"]}
```

- `items` is a list of sections, the same shape as a Sidebar's. A section is `{"label",
  "items"}`; without `label` its entries have no heading. Each entry is `{"id", "label"}` with
  optional `icon` (a bundled icon name), `keywords` (a list of extra terms the search matches),
  `checked` (draws a check at the end of the row) and `disabled`.
- `id` is a string chosen by the client and must be unique across the whole palette; a repeated
  one, an entry without `id` or `label`, a section without `items`, an icon that is not bundled
  or a mistyped field is `invalid_props`, and the operation is not applied.
- The palette filters the entries by the query itself (label and `keywords`, ignoring case); the
  query is not reported. Up and down move the highlight, Enter or a click confirms it.
- `confirm` (opt-in) reports the confirmed entry's `id`. A palette holds no value: nothing is
  kept in the host, and a disabled entry cannot be confirmed.
- A modal palette is a `Command` inside a `Dialog` (section 14): Escape first clears the query,
  and with an empty query it reaches the dialog, which closes and sends its `cancel`.

```json
{"type":"event","id":40,"event":"confirm","value":"quit"}
```

`simulate` takes the same `value`: `{"op":"simulate","id":40,"event":"confirm","value":"quit"}`.
An unknown `id` is `invalid_operation`; a disabled one sends nothing.

`PROTOCOL_VERSION` stays at 1: `Command` is a new node type, so an older host answers
`unknown_type` to it and nothing else changes.

The transcript in `conformance/command.txt` is the reference for its props and events.

## 22. Settings

A settings screen: a sidebar of pages, with a search field, and the groups of fields of the
selected page. Its content is data in props and it takes no children. The sidebar and the page
split the node's box, so give it a size (`size-full`, `flex-1`, `h-96`).

```json
{"op":"create","id":50,"type":"Settings","props":{"class":"size-full","pages":[
  {"title":"General","icon":"settings","groups":[
    {"title":"Look","items":[
      {"key":"dark","label":"Dark mode","type":"switch","value":true},
      {"key":"font","label":"Font","type":"dropdown","value":"mono",
       "options":[{"value":"mono","label":"Mono"},{"value":"sans","label":"Sans"}]}
    ]}
  ]},
  {"title":"Editor","groups":[{"items":[
    {"key":"name","label":"Name","type":"input","value":"","description":"Shown in the title"},
    {"key":"size","label":"Size","type":"number","value":14,"min":8,"max":32,"step":1},
    {"key":"lines","label":"Line numbers","type":"checkbox","value":false,"disabled":true}
  ]}]}
]},"events":["change"]}
```

- `pages` is a list of `{"title", "icon"?, "groups"}`; `icon` is a bundled icon name. A group is
  `{"title"?, "items"}`; a group without `title` has no heading.
- An item is `{"key", "label", "type", "value"}` with optional `description` and `disabled`.
  `type` is one of:

  | `type`     | `value`  | Extra fields |
  |------------|----------|--------------|
  | `switch`   | boolean  | — |
  | `checkbox` | boolean  | — |
  | `input`    | string   | — |
  | `dropdown` | string, one of the options' `value` | `options`: `[{"value", "label"}]` (required) |
  | `number`   | number   | `min`, `max` (`min` not above `max`), `step` (above 0, default 1) |

- `key` is a string chosen by the client and must be unique across all pages. A repeated key, a
  missing field, an unknown `type`, a `value` that does not fit its `type`, an icon that is not
  bundled or a mistyped field is `invalid_props`, and the operation is not applied.
- The search field filters the items by label and description; the selected page and the query
  stay in the host and are not reported.
- When the user changes a field, the host keeps the new `value` in `pages` (there is no
  `controlled` mode) and `change` (opt-in) reports `{"key", "value"}`. A text field reports each
  edit; a number field only reports values clamped to `min`..`max`. A `setProps` of `pages`
  replaces the values the host kept.

```json
{"type":"event","id":50,"event":"change","value":{"key":"dark","value":false}}
```

`simulate` takes the same `value`:
`{"op":"simulate","id":50,"event":"change","value":{"key":"dark","value":false}}`. An unknown
`key` or a `value` that does not fit the field is `invalid_operation`; a disabled field, or the
value the field already has, sends nothing.

Left out: custom field elements, reset buttons, group footers and group variants.

`PROTOCOL_VERSION` stays at 1: `Settings` is a new node type, so an older host answers
`unknown_type` to it and nothing else changes.

The transcript in `conformance/settings.txt` is the reference for its props and events.

## 23. Simple components

Seven small nodes: a toggle button, a split button with a menu, a rich text label, an empty
state, loading text, a framed text field and a copy button.

```json
{"op":"create","id":40,"type":"Empty","props":{"icon":"folder-open","title":"No files","description":"Drop a file here"}}
{"op":"create","id":41,"type":"Button","props":{"label":"Choose a file"}}
{"op":"append","parent":40,"child":41}
{"op":"create","id":42,"type":"Toggle","props":{"label":"Bold","icon":"asterisk","variant":"outline","checked":false},"events":["change"]}
{"op":"create","id":43,"type":"DropdownButton","props":{"label":"Save","variant":"primary","items":[{"id":"save-as","label":"Save as"},{"id":"discard","label":"Discard","disabled":true}]},"events":["click","menu"]}
{"op":"create","id":44,"type":"InputGroup","props":{"value":"example.com","prefix":"https://","suffixIcon":"check"},"events":["submit"]}
{"op":"create","id":45,"type":"Label","props":{"content":"Total","secondary":"(12)","highlight":"12"}}
{"op":"create","id":46,"type":"Shimmer","props":{"content":"Loading...","duration":2}}
{"op":"create","id":47,"type":"Clipboard","props":{"value":"https://example.com","a11y":{"label":"Copy the address"}},"events":["copied"]}
```

- **Toggle** — a button that stays pressed: `checked`, `label`, `icon` and a `tooltip` that also
  names it for a screen reader; `variant` is `ghost` (default) or `outline`. Its children are
  drawn after `label`/`icon`. A click sends `change` with `checked`, and the synced and
  controlled modes of section 5 apply, as with `Checkbox`. It takes no Tab stop.
- **DropdownButton** — an action and its menu in one control: a `Button` half (`label`, `icon`,
  `variant`, `outline`, `tooltip`) and a caret that opens `items`, a menu list in the format of
  section 10 (`menuLabel` names the caret for a screen reader). A click on the left half sends
  `click`; choosing an item sends `menu` with its `id`. It takes no children.
- **Label** — `content` with an optional `secondary` text after it (muted) and `masked` (dots
  instead of characters); `highlight` marks **every** occurrence of that substring, ignoring
  case. It is not drawn when a character of the texts changes its UTF-8 length when lowercased
  (such as the Kelvin sign `K` or `İ`): gpui-component would crash on it.
- **Empty** — a centered empty state: a dashed frame whose header is `icon` (in a muted box),
  `title` and `description`, with the children in a content slot under it (usually a `Button`).
  It fills the box it is given (`flex-1`, `min-w-0`), so size its parent or give it `class`.
- **Shimmer** — `content` with a moving highlight: `duration` in seconds (default 2), `spread`
  (the highlight's half-width, 0.3 by default), `reverse` and `once`. It animates on its own —
  no `animate-*` class — and shows the text still when the system asks for reduced motion. The
  crate clamps `spread` to 0.05–1 and a negative `duration` to 1 ms; a `duration` too large to
  represent keeps the default.
- **InputGroup** — a framed text field: `value` (applied like an `Input`'s), `placeholder`,
  `prefix`/`prefixIcon` on the left and `suffix`/`suffixIcon` on the right (text and a bundled
  icon; a side with neither adds nothing), `readonly`, `invalid`, `disabled` and `controlled`.
  `change` and `submit` work as in an `Input`, and `a11y.label` names the group (its `role` is
  ignored). The sides are text and an icon only: a button inside the frame is not part of this
  node.
- **Clipboard** — a small icon button that copies `value`. A click copies it and sends `copied`
  with that text; the icon turns into a check for two seconds, and a second click in that window
  does nothing. `tooltip` is a hint and `a11y.label` names the button (its own default name is a
  translated "Copy").

The `items` of a `DropdownButton` are the menu list of section 10, submenus included, and a
disabled or unknown item behaves as it does there.

`simulate` covers the actions: `change` on a `Toggle`, `change` and `submit` on an `InputGroup`,
`click` and `menu` on a `DropdownButton`, and `copied` on a `Clipboard`. Without a window
(`--no-window`) nothing is copied: the event is reported and the system clipboard is not touched,
as with `readClipboard` and the native dialogs. `simulate copied` has no two-second wait, so two
calls send two events.

`PROTOCOL_VERSION` stays at 1: the seven are new node types, so an older host answers
`unknown_type` to them and nothing else changes.

The transcript in `conformance/simple.txt` is the reference for their props and events.

## 24. Overlays

Four nodes over the contents of a window: a confirmation, a side panel, and the two panels a
trigger opens.

```json
{"op":"create","id":60,"type":"AlertDialog","props":{"open":true,"title":"Delete 3 files?","description":"This cannot be undone.","icon":"circle-alert","okText":"Delete","cancelText":"Cancel"},"events":["ok","cancel"]}
{"op":"create","id":62,"type":"Sheet","props":{"open":true,"side":"left","title":"Filters","size":320},"events":["close"]}
{"op":"create","id":64,"type":"Popover","props":{"anchor":"bottom-right","arrow":true},"events":["change"]}
{"op":"create","id":67,"type":"HoverCard","props":{"openDelay":300,"closeDelay":200},"events":["change"]}
```

- **AlertDialog** — a confirmation in the dialog layer of section 14: `open` (default `false`),
  `title`, `description`, `icon` (a bundled icon, as in `Empty`) and `okText` (default `OK`).
  `cancelText` is what adds the Cancel button, as in a `Dialog`. Its children are the body, under
  the description, and `class` styles it. `ok`, `cancel` (also Escape) are opt-in events that
  close it and set `open` to `false` in the host, as with a `Dialog`; there is no X and no
  dismiss-by-click, so neither cancels. An open `AlertDialog` shows over an open `Dialog`, and
  closing it brings the `Dialog` back, with its contents as they were: that is how a confirmation
  is put on top of another dialog. The two share one layer, so a window still shows one dialog at
  a time (the open `AlertDialog` wins; among equals, the lowest id).
- **Sheet** — a panel that slides in from `side` (`right` by default; `left`, `top`, `bottom`):
  `open`, `title` and `size` in pixels (default 350; the width on the sides, the height above and
  below). Its children are the scrollable body and `class` styles it. Closing it with its X,
  Escape or a click on the dimmed background sends `close` and sets `open` to `false`; closing it
  with `open: false` sends nothing. The panel is resizable by its edge (the crate's default) and
  its overlay and close-on-overlay behaviour are not props. One sheet per window: the open one
  with the lowest id.
- **Popover** — a panel anchored to its **first child**, the trigger; the rest of the children are
  the panel. `open` (default `false`) lives in the host and is synchronized like a `Checkbox`'s
  `checked`: the user opening or closing it (a click on the trigger, Enter or Space with the focus
  in it, Escape, a click outside) sends `change` with `checked` and updates `open`, and a
  `setProps` with another `open` wins. `anchor` (default `top-left`) names where the panel goes:
  `top-*` opens below the trigger and `bottom-*` above it, aligned left, centre or right;
  `left-center` and `right-center` open to the sides. `arrow` draws a triangle pointing at the
  trigger. `class` styles the **trigger's slot**, not the panel: put `w-full` there for a trigger
  that fills the space its parent gives it, and style the panel with the nodes inside it. Only the
  static `class` utilities reach that slot (`hover:` and `transition` need the element itself, so
  they do not). The trigger keeps its own `click`, and that click does not bubble up; a disabled
  or loading `Button` does not open the popover. For the keyboard to open it the trigger must be
  focusable (`Button`, `Input`): a `Text` or a plain `div` trigger is mouse-only.
- **HoverCard** — the same shape as a `Popover`, opened and closed by the mouse alone: the first
  child is the trigger. `openDelay` and `closeDelay` are milliseconds (600 and 300 by default) and
  `anchor` (default `top-center`) works as in a `Popover`. It has no `open` prop — the host does
  not open or close it, and there is no `simulate` for it — its `change` with `checked` comes from
  a real hover only. It does not take the focus, so neither the keyboard nor a screen reader
  reaches it: do not put information there that is nowhere else.

Without children these two draw nothing (the first child is the trigger) and the host warns once on
stderr; with one child they still open an empty panel with the crate's padding.

The `a11y` of the node itself is ignored by the four: an `AlertDialog` and a `Sheet` read their
`title`, and in a `Popover`/`HoverCard` the reader reads the `a11y` of the trigger child.

`simulate` covers `ok`/`cancel` on an open `AlertDialog` (as on a `Dialog`), `close` on an open
`Sheet`, and `change` on a `Popover` (which toggles `open`, as a click does). A `HoverCard` has
none: its `change` only comes from the mouse in a real window.

`PROTOCOL_VERSION` stays at 1: the four are new node types, so an older host answers `unknown_type`
to them.

The transcript in `conformance/overlays.txt` is the reference for their props and events.

## 25. Form controls with state

Four form controls that keep what the user picks in the host, like `Select` and `Calendar`: the
value lives in the node's `value` and every change the user makes is reported with `change`.

```json
{"op":"create","id":70,"type":"DatePicker","props":{"value":"2024-08-03","placeholder":"Pick a day"},"events":["change"]}
{"op":"create","id":71,"type":"DatePicker","props":{"range":true,"value":"2024-08-01/2024-08-15"},"events":["change"]}
{"op":"create","id":72,"type":"TimeField","props":{"value":"09:30"},"events":["change"]}
{"op":"create","id":73,"type":"ColorPicker","props":{"value":"#3366ff","palette":["#ff0000","#00ff00"]},"events":["change"]}
{"op":"create","id":74,"type":"Combobox","props":{"options":[{"value":"rs","label":"Rust"},{"value":"go","label":"Go"}],"value":["rs"],"searchable":true},"events":["change"]}
```

- **DatePicker** — a field that opens a calendar. `value` is a day, `YYYY-MM-DD`; with
  `range: true` it is an ISO 8601 interval, `YYYY-MM-DD/YYYY-MM-DD`, with the start not after the
  end. A range is reported once both ends are picked. The field has a clear button, which sends
  `change` with `value: null`. `placeholder` shows while it is empty.
- **TimeField** — a time typed by segments (hours, minutes, seconds). `value` is `HH:MM` (24 hours);
  with `seconds: true` it is `HH:MM:SS`. It has no empty state: without `value` it shows `00:00`,
  and the user cannot clear it.
- **ColorPicker** — a field with a swatch and the hex value that opens a palette, hue, saturation,
  lightness and alpha sliders and a hex box. `value` is `#RRGGBB` or `#RRGGBBAA` (the `#` is
  required, any case); the host reports `#RRGGBB` in capitals, and `#RRGGBBAA` when the color is
  translucent. `palette` is a list of colors in the same form, shown first. Clearing it sends
  `value: null`. It has no `disabled`: gpui-component cannot disable its color pickers.
- **Combobox** — a list that picks several options: `options` as in a `Select`, `value` the list of
  chosen option values. Each toggle (adding or removing one) sends `change` with the whole list;
  values that are not among the options are not shown. `searchable` adds a search box and
  `placeholder` shows while nothing is chosen. For a single choice with search, use a `Select`
  with `searchable`.

`range` and `seconds` fix the form of `value`, so a `value` is checked against the node as the
operation leaves it: `setProps` with `range: true` on a node whose `value` is a single day is
`invalid_props` (send the new `value` with it). A `value` that does not fit (`2024-02-30`, an
inverted range, `24:00`, `3366ff`), an empty string, or a `palette` color that is not hex is
`invalid_props` and the operation is not applied. Unlike `Calendar`, which shows no day for a
date it cannot read, these four reject it.

Like a `Select`, the four have two modes (section 5): by default the host keeps the new value and
reports it; with `controlled: true` it only reports it and keeps showing the client's `value` until
a `setProps` confirms it.

`simulate` covers `change` on the four, with the `value` the user would pick: a string for the first
three (`null` clears a `DatePicker` or a `ColorPicker`), and a list of strings for a `Combobox`. A
value that does not fit the node or an unknown option is `invalid_operation`; a value of the wrong
type is `invalid_json`. The value the node already has sends nothing, as in section 5, and so does
a `Combobox` list that adds a disabled option (removing one is allowed: the clear button does it).

`PROTOCOL_VERSION` stays at 1: the four are new node types, so an older host answers `unknown_type`
to them.

The transcript in `conformance/form-state.txt` is the reference for their props and events.

## 26. Chat

Four presentational nodes for a conversation, the header and footer of a `Message`, the
scrolling list that holds them, and a step-by-step questionnaire. They are drawn from their
props every frame; a `MessageScroller` keeps its scroll position in the host and a
`Questionnaire` its answers. An `Attachment`'s upload status is the client's to change.

```json
{"op":"create","id":80,"type":"Message","props":{"alignment":"end","avatarName":"Ada"}}
{"op":"create","id":81,"type":"MessageHeader"}
{"op":"create","id":82,"type":"text","props":{"content":"Ada · 10:42"}}
{"op":"create","id":83,"type":"Bubble","props":{"variant":"filled"}}
{"op":"create","id":84,"type":"text","props":{"content":"Here is the report."}}
{"op":"create","id":85,"type":"MessageFooter"}
{"op":"create","id":86,"type":"Button","props":{"label":"Copy","variant":"ghost"},"events":["click"]}
{"op":"append","parent":81,"child":82}
{"op":"append","parent":83,"child":84}
{"op":"append","parent":85,"child":86}
{"op":"append","parent":80,"child":81}
{"op":"append","parent":80,"child":83}
{"op":"append","parent":80,"child":85}
{"op":"create","id":87,"type":"Marker","props":{"content":"Thinking","loading":true,"loadingStyle":"shimmer"}}
{"op":"create","id":88,"type":"Attachment","props":{"status":"uploading","progress":45,"title":"report.pdf","description":"2.4 MB"},"events":["click","remove"]}
{"op":"create","id":89,"type":"MessageScroller","props":{"class":"flex-1","jumpLabel":"Ir al último"}}
{"op":"append","parent":89,"child":80}
{"op":"append","parent":89,"child":87}
```

- **Bubble** — a chat bubble: its children are the content. `variant` is the surface (`filled`
  by default; `ghost` has no surface or padding and takes the whole width). `alignment` puts it at
  the `start` or `end` of the row; unset, it follows the `Message` that holds it. It is at most
  80% as wide as its parent; with `a11y`, the box that carries it is a full-width column, so it
  keeps that width and its side. Reactions are not part of this node (put a row of `Button`s in the
  message's footer instead).
- **Message** — a message row: an avatar, a header, the content and a footer, aligned to the
  `start` (default) or the `end` (the avatar moves to that side too). `avatarName`/`avatarSrc`
  draw an `Avatar` (its `name` and `src`); without either there is no avatar. A
  **MessageHeader** child is the header (sender, time) and a **MessageFooter** child the footer
  (state, actions); both lay out their own children in a row with small muted text, take their
  own `class`, and the last of each wins. Every other child is the content, usually one or more
  `Bubble`s. With a `ghost` bubble the header and footer lose their side inset so they line up
  with its text. A `MessageHeader` or `MessageFooter` outside a `Message` draws nothing, like a
  `Tab` outside `Tabs`. Group messages with a `div` and `class` (`flex flex-col gap-4`).
- **Marker** — a compact status row in a conversation: `content` (text), an `icon` before it,
  and children after the text (inside the same content slot). `variant` is `plain` (default),
  `separator` (a line on each side, label centered) or `border` (a line under it); `alignment`
  overrides where the content sits, and a separator keeps only the line on the far side of a
  `start` or `end` label. `loading` adds a spinner (unless there is an `icon`) or, with
  `loadingStyle: "shimmer"`, a moving highlight over `content` (children only fade; the shimmer
  stops when the system asks for reduced motion). A marker that reports progress can be
  announced with `a11y: {"role": "status"}`.
- **Attachment** — a file or picture card. `status` is `complete` (default), `pending` (dashed
  border), `uploading`, `processing`, or `failed` (red border); `progress` (0–100, clamped) draws
  a ring and a bar along the bottom and is added to `description`, only while `uploading`.
  `title` and `description` are text (the title shimmers while in progress; the description
  turns red when `failed`). The picture is `src` or `data`, exactly as in an `Image` (`data`
  wins and must be standard base64); without one the media shows a file icon. `axis: "vertical"` turns the chip into a card with the picture on top
  (only the picture, without `title`/`description`, makes a square tile). `tooltip` shows while
  the card is hovered (the reason an upload failed, for example). The children are actions
  (usually `Button`s): they sit on the trailing edge and their clicks never reach the card.

  The card's own controls exist only while their event is subscribed: `click` makes the whole
  card clickable, `remove` shows a remove button on its upper corner (on hover), and `retry`
  shows a retry control, only while `failed` (a round button on the picture or the file icon,
  and a link after `description`). None carries a value. `simulate` sends all three;
  a `retry` on an attachment that is not `failed` sends nothing, since the button is not there.
- **MessageScroller** — a virtual list of messages: each child is a row (usually a `Message` or
  a `Marker`), rows can have any height, and only the ones in view are built, so tens of
  thousands of messages cost little. It fills the box it is given, so size its parent or give
  it `class` (`flex-1`). It starts at the end and stays there as rows are added or the last one
  grows (a streamed answer); when the user scrolls up it stays put, and a round button
  (`jumpButton`, on by default) brings it back to the end. `jumpLabel` is that button's tooltip
  and name (the crate's default is the English "Jump to latest"); `scrollbar` (on by default)
  shows a vertical scrollbar. The scrollbar is approximate: rows that were never drawn count as
  zero height, so its thumb changes size as the user scrolls through the list, and dragging it
  moves the content in small jumps (the wheel and the jump button are smooth). Adding, removing or moving rows keeps the reading position: the
  host compares the new children with the old ones and tells the list only what changed in
  between. The list has the `log` role. It has no events.
- **Questionnaire** — questions shown one at a time, with progress ("Question 1 of 3"), and
  Previous, Skip, Next and Submit buttons that appear when they apply. `items` lists the
  questions: `{name, label, description?, required?, multiple?, disabled?, input?, choices?}`,
  where `choices` are `{value, label, description?, disabled?, selected?}` (radio buttons, or
  checkboxes with `multiple`) and `input: true` adds a free-text field. `name`s and each
  question's choice `value`s must be unique, and a question without `multiple` can have at
  most one `selected` choice; anything else is `invalid_props`. A `selected` choice is an
  answer from the start. Choosing in a single-choice question moves to the next one on its own;
  a `required` question cannot be skipped, and an optional one must be answered or skipped
  before Submit. On the keyboard, up and down move between answers, left and right between
  questions, and Enter confirms a filled answer.

  Every answer is reported with `change`, whose value is `{name, status, choices, text}`:
  `status` is `answered`, `skipped` or `unanswered` (cleared), `choices` the chosen values and
  `text` the free text or `null`; typing sends one `change` per edit, as an `Input` does.
  `submit` sends the list of those answers, in question order (disabled questions left out).
  The host keeps the answers in the node (`answers`, by name, visible in the `--no-window`
  dump), and setting different `items` starts over (the same ones again keep the answers). Without a window, `simulate change` takes
  `{name, choices?, text?}` (an answer the user could give, or `invalid_operation`) and
  `simulate submit` sends `submit` only when every question is answered; skipping exists only
  in a window. The client cannot set the answers, and validation beyond `required` is the
  client's, after `submit`.

`PROTOCOL_VERSION` stays at 1: the eight are new node types, so an older host answers
`unknown_type` to them and nothing else changes.

The transcript in `conformance/chat.txt` is the reference for their props and events.

## 27. List

A selectable list with the keyboard, an optional search field and incremental loading. Its rows
are **`ListItem` children** (the pattern of `Tab`, section 15), so a row can hold any nodes:

```json
{"op":"create","id":1,"type":"List","props":{"class":"h-64","value":"b","searchable":true,"placeholder":"Search fruit","hasMore":true},"events":["change","confirm","query","loadMore"]}
{"op":"create","id":2,"type":"ListItem","props":{"value":"a"}}
{"op":"create","id":3,"type":"text","props":{"content":"Apple"}}
{"op":"append","parent":2,"child":3}
{"op":"append","parent":1,"child":2}
{"type":"event","id":1,"event":"change","value":"a"}
{"type":"event","id":1,"event":"confirm","value":"a"}
```

- A `ListItem` is `value` —required and not empty, it is what the events report— and
  `disabled`. Values should be unique within a list (with a repeat, the first row wins). Its
  children are laid out in a row. Other children of a `List` are not drawn.
- A screen reader reads a row by its `a11y.label`: gpui-component gives the row the list item role
  but no name, so the host puts `a11y` on the row's content (with the `label` role unless another is
  given). While the list itself has the focus, moving the selection moves the reader to the
  selected row. With `searchable` the focus stays in the search field, so the reader stays there
  too and reads the rows only when browsing (scan mode). A row without `a11y.label` has no name.
- Every row has the height of the first one (gpui-component measures one row); what is taller
  is cut. The list virtualizes its rows and fills its container, so give it a height (`h-64`,
  `flex-1`) with `class`. The client still sends every row as nodes, as with `VirtualList`.
- `value` is the selected row's `value`; one that is not among the rows selects nothing. The
  arrow keys (passing over disabled rows) and a click send `change` with the row's `value`, and
  Escape sends `change` with `null`. `value` follows the synced and controlled modes of
  section 5 (`controlled`). The list passes Escape on to what holds it.
- `confirm` reports the row's `value` on Enter or a click (a click on another row sends its
  `change` first). A disabled row is neither selected nor confirmed.
- `searchable` shows a search field above the rows (`placeholder` is its hint). Each edit sends
  `query` with the text, trimmed; the same text twice is sent once. The host does not filter:
  the client sends the rows that match (and, to keep Enter useful, usually sets `value` to the
  first one). While typing, the list highlights its first row until the new rows arrive.
- With `hasMore`, scrolling to within `loadMoreThreshold` rows of the end (a whole number,
  default 20) sends `loadMore`, once per row count: the client appends rows, and clears
  `hasMore` when there are no more. `loading` replaces the rows with gpui-component's skeleton.
  A list without rows shows gpui-component's empty state.
- `focus` (section 12) works on a List, so the arrow keys move the selection.
- Without a window, `simulate change` takes a row's `value` or `null`, `simulate confirm` a
  row's `value` (it selects it first), `simulate query` the text and `simulate loadMore` nothing
  (sent only with `hasMore`). An unknown row is `invalid_operation`; a disabled one does nothing.

`PROTOCOL_VERSION` stays at 1: `List` and `ListItem` are new node types. The transcript in
`conformance/list.txt` is the reference for their props and events.
