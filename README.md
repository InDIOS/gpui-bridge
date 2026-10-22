<p align="center">
  <img width="611" height="367" alt="gpui-bridge logo" src="https://github.com/user-attachments/assets/35c33e2e-2844-4091-98ca-5a3822cde259" />
</p>

<h1 align="center">gpui-bridge</h1>

Native desktop UIs with [GPUI](https://www.gpui.rs/) (the UI framework of the Zed editor) and
[gpui-kit](https://gpui-kit.com/), driven from another language.

A Rust **host** owns the window, the event loop and the component tree. A **client** in your
language describes the UI as operations on that tree (create a node, set props, append a
child, remove) and receives events (click, change…). They talk over stdin/stdout with JSON
Lines, so any language that can start a process can be a client.

```ts
import { createApp } from "gpui-bridge";

const app = await createApp();
let count = 0;
const label = app.text({ content: "0", class: "text-xl font-bold" });
const add = app.button({ label: "Add", variant: "primary" }).on("click", () => {
  label.set({ content: String(++count) });
  app.commit();
});
app.window({ title: "Counter", width: 320, height: 200, root: app.div({ class: "flex flex-col items-center gap-4 p-6 size-full" }).append(label, add) });
app.commit();
```

Styles are Tailwind-like class strings (`"flex gap-2 p-4 bg-muted"`) that use the theme's
colors, so light and dark mode and custom themes come for free.

**Status:** Windows only so far (GPUI also runs on macOS and Linux, but the host has not been
built or tested there). Protocol version 1.

## What it has

- **Nodes:** `div`, `text`, `Button`, `Input`, `Textarea`, `Checkbox`, `Switch`,
  `RadioGroup`, `Select`, `Slider`, `Icon` (Lucide), `Image` (a file path or bytes), `MenuBar`, `Dialog`,
  `VirtualList`, `Table`, `Tabs`, and most of gpui-kit's Basic, Form and Layout groups:
  `Alert`, `Badge`, `Tag`, `Kbd`, `Progress`, `Spinner`, `Skeleton`, `Rating`, `Pagination`,
  `Collapsible`, `Accordion`, `Avatar`, `Stepper`, `NumberInput`, `OtpInput`, `Form`/`Field`,
  `Tab` inside `Tabs`, `GroupBox`, `DescriptionList`, `StatusBar`, `Toolbar`, `Resizable`, plus
  `Calendar`, `Tree`, `Diff`, `Carousel`, `Sidebar` and `Chart` (line, bar and pie).
  `Editor` and `Speech` are left out (they need `tree-sitter` and the system recognizer), and
  `Dock`, `Command`, `Settings` and `List` are not mapped yet.
- **Accessibility:** every node takes an `a11y` prop (an id, a role, a label, a description and
  the ARIA-like state: checked, selected, expanded, values and ranges), reported to screen
  readers through AccessKit; the window root and the controls name themselves too.
- **Windows:** several windows, frameless windows with a custom title bar, initial position,
  minimum size, minimize/restore, window events (`focus`, `blur`, `resize`, `move`,
  `visibility`), light/dark and custom themes.
- **Interaction:** context menus and a menu bar with shortcuts, app shortcuts, `keyDown`,
  focus, double click, tooltips, toasts, dropping files.
- **System:** tray icon with a native menu, open/save dialogs, clipboard, opening URLs and
  files, notifications in the Windows notification center.
- **Styling:** layout, spacing, sizes, borders, text, theme colors, `hover:`, opacity,
  transitions and `animate-spin`/`animate-pulse`.

## Clients

| Folder | Package | Language |
|---|---|---|
| [`packages/client-node/`](packages/client-node/) | `gpui-bridge` | TypeScript for Node.js 22.6+; the same core runs on [txiki.js](https://github.com/saghul/txiki.js) |
| [`packages/client-react/`](packages/client-react/) | `gpui-bridge-react` | A React renderer on top of `gpui-bridge` (`<Div>`, `<Button onClick>`…) |
| [`packages/client-python/`](packages/client-python/) | `gpui-bridge` | Python 3.10+, standard library only |

A client for another language only needs [PROTOCOL.md](docs/PROTOCOL.md); the transcripts in
[`conformance/`](conformance/) show the expected messages.

## Repository layout

```text
packages/host/          the Rust host (workspace member; ships inside the client packages)
packages/client-node/   npm package `gpui-bridge`
packages/client-react/  npm package `gpui-bridge-react`
packages/client-python/ PyPI package `gpui-bridge`
examples/               runnable examples, installed against the packages like a user would
conformance/            protocol transcripts, shared by the host and the clients
benchmarks/             transport, big-tree and payload measurements
docs/                   protocol specification, plan, history, releasing notes
scripts/                host staging and the `pnpm dev` runner
```

## Getting started (from this repository)

Requirements: Rust (stable, with `rustup`), Visual Studio Build Tools with the C++ workload,
Node.js 22.6+ with pnpm, and Python 3.10+ for the Python client. The first build of GPUI takes
several minutes.

```sh
pnpm bootstrap                                 # pnpm install + debug host + TypeScript builds
pnpm dev counter                               # opens examples/counter.ts
pnpm dev counter -- --debug                    # host logs on stderr
python examples/counter.py
```

Add `RUST_LOG=debug` to `--debug` to also see, once a second, how many frames a window draws
and what its own pass costs (`window 1: 60.0 frames/s, 0.2 ms and 14 nodes per pass`).

The clients look for the host in this order: an explicit path option, `$GPUI_BRIDGE_HOST`,
the host bundled in the package, then a Cargo build in `target/debug` or `target/release`
of this repository (searched upwards, so the layout of the monorepo is not part of the
contract).

The host asks for an 8 MB stack for its main thread (`host/build.rs`, the same Rust gives on
Linux): a debug build spends tens of KB of stack per level of a nested tree, and Windows'
1 MB default overflowed on deep trees (`STATUS_STACK_OVERFLOW`, exit code 3221225725).

## Examples

In [`examples/`](examples/) (run with `pnpm dev <name>`, which hands the repository's host to the
example; they import `gpui-bridge` like an installed package does):

| Example | Shows |
|---|---|
| `counter.ts`, `counter.py`, `counter-txiki.js` | The same counter in Node, Python and txiki.js |
| `todo.ts` | Input with submit, checkboxes, a list that changes |
| `theme.ts` | A custom theme file, light and dark |
| `frameless.ts` | A window with its own title bar |
| `windows.ts` | A main window that opens a settings window |
| `menus.ts` | Menu bar and context menu (`--frameless` puts the menu bar in the title bar) |
| `tray.ts` | Tray icon; closing the window keeps the app in the tray |
| `animations.ts` | Spinner, pulse and color transitions |
| `forms.ts` | Switch, RadioGroup, Select, Slider, Textarea; a controlled slider |
| `keyboard.ts` | Shortcuts, a list moved with the arrow keys, focus |
| `image-bytes.ts` | An `Image` from a path and from base64 bytes (`data`), re-sent on click |
| `system.ts` | Open/save dialogs, clipboard, opening URLs and files, dropping files |
| `overlays.ts` | A dialog, toasts and tooltips |
| `components.ts` | The Basic and Layout components, a `Form`, a `Tabs` of `Tab` nodes, a `Resizable` split and accessibility |
| `data.ts` | A 10,000-row `VirtualList` and a sortable 10,000-row `Table` |
| `advanced.ts` | A `Tree` with expansion, a `Diff` of a patch and a `Carousel` |
| `dashboard.ts` | A `Sidebar`, three `Chart`s (1,000 points, bars, a pie) and a 10-level tree |

In [`examples/react/`](examples/react/): `todo.tsx` (`pnpm dev react/todo`) and a file browser,
`files.tsx` (`pnpm dev react/files`).

## Tests

```sh
pnpm test                             # everything below, in order
pnpm test:host                        # unit tests + conformance transcripts + --no-window tests
pnpm test:node
pnpm test:react
pnpm test:python
```

The client tests run the real host without a window (`--no-window`). `pnpm test:host` runs the
transcripts in `conformance/`, which every client must reproduce (PROTOCOL.md §9).

## Benchmarks

[`benchmarks/`](benchmarks/) measures the transport, big trees and image payloads:

```sh
pnpm bench                            # or: pwsh -File benchmarks/run.ps1
```

See [benchmarks/README.md](benchmarks/README.md) for what each number means and its limits.

## Packaging

Each client is published with the host inside it as an asset (React has no binary: it depends
on `gpui-bridge`, which brings one). The host is compiled **once** per platform and staged by
`scripts/stage-host.mjs`; nothing is built twice.

```sh
pnpm pack:node                        # release host -> packages/client-node/bin/ -> pnpm pack
pnpm wheel                            # release host -> gpui_bridge/bin/ -> platform-tagged wheel
```

Both scripts print the path of the artifact. The optimized host is about 15 MB (6 MB zipped).
With txiki.js, a whole app is about 19 MB.

## Documents

- [PROTOCOL.md](docs/PROTOCOL.md): the protocol, for client authors (English).
- [GPUI_BRIDGE_PLAN.md](docs/GPUI_BRIDGE_PLAN.md): the project plan, design decisions and next
  steps (Spanish).
- [GPUI_BRIDGE_COMPONENTS.md](docs/GPUI_BRIDGE_COMPONENTS.md): the gpui-kit components not mapped
  yet —why, what each would cost, and its impact on host, protocol and clients (Spanish).
- [HISTORIAL.md](docs/HISTORIAL.md): what each development phase did and how it was verified
  (Spanish).
- [CONTRIBUTING.md](CONTRIBUTING.md): environment, layout and the checks a change must pass.
- [RELEASING.md](docs/RELEASING.md): how a version is cut and what each package contains.
- [adr/0001-monorepo.md](docs/adr/0001-monorepo.md): why the repository is laid out this way.
