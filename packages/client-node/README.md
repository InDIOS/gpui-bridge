# gpui-bridge

Native desktop UIs with [GPUI](https://www.gpui.rs/) (the UI framework of the Zed editor) and
[gpui-component](https://github.com/longbridge/gpui-component), driven from JavaScript or
TypeScript. This package is the Node.js client; a Rust host owns the window, the event loop and
the component tree, and they talk over stdin/stdout with JSON Lines.

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

## Install

```sh
npm install gpui-bridge
```

The package carries an optimized `gpui-bridge-host` binary for **Windows x64** in `bin/`, so it
needs no Rust toolchain at runtime. Node.js 22.6+ (or [txiki.js](https://github.com/saghul/txiki.js)
through `gpui-bridge/dist/txiki.js`). On another platform, set `GPUI_BRIDGE_HOST` to a host built
from the repository (the host has only been built and tested on Windows so far).

Styles are Tailwind-like class strings that use the theme's colors, so light and dark mode and
custom themes come for free. Props, events and every node type are typed.

## Host lookup

1. `createApp({ hostPath })`
2. `$GPUI_BRIDGE_HOST`
3. `bin/gpui-bridge-host[.exe]` inside this package
4. A Cargo build in `target/debug` or `target/release`, searched upwards (a checkout)

`bin/host.json` records the host version, protocol version and target the package was built
with; include it in a bug report.

## Links

- [Protocol specification](https://github.com/InDIOS/gpui-bridge/blob/main/docs/PROTOCOL.md),
  for writing a client in another language.
- [React renderer](https://www.npmjs.com/package/gpui-bridge-react) on top of this client.
- Examples and the rest of the repository:
  [InDIOS/gpui-bridge](https://github.com/InDIOS/gpui-bridge).

Protocol version 1. MIT licensed.
