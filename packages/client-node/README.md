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

The package pulls in an optimized `gpui-bridge-host` binary for the machine installing it: the
npm package `gpui-bridge-<platform>-<arch>` (`gpui-bridge-win32-x64`, `gpui-bridge-linux-x64`,
`gpui-bridge-darwin-arm64`) is an optional dependency, so only the host this machine can run is
downloaded. No Rust toolchain is needed at runtime — except on a platform without a package
yet: set `GPUI_BRIDGE_HOST` to a host built from the repository. Node.js 22.6+ (or
[txiki.js](https://github.com/saghul/txiki.js) through `gpui-bridge/dist/txiki.js`).

Styles are Tailwind-like class strings that use the theme's colors, so light and dark mode and
custom themes come for free. Props, events and every node type are typed.

## Host lookup

1. `createApp({ hostPath })`
2. `$GPUI_BRIDGE_HOST`
3. `gpui-bridge-<platform>-<arch>/bin/gpui-bridge-host[.exe]`, the platform package installed
   with this one (`defaultHostPath()` reports the path that wins)
4. A Cargo build in `target/debug` or `target/release`, searched upwards (a checkout)

The platform package's `bin/host.json` records the host version, protocol version and target it
was built for; include it in a bug report.

## Links

- [Protocol specification](https://github.com/InDIOS/gpui-bridge/blob/main/docs/PROTOCOL.md),
  for writing a client in another language.
- [React renderer](https://www.npmjs.com/package/gpui-bridge-react) on top of this client.
- Examples and the rest of the repository:
  [InDIOS/gpui-bridge](https://github.com/InDIOS/gpui-bridge).

Protocol version 1. MIT licensed.
