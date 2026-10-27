# gpui-bridge (Python)

Native desktop UIs with [GPUI](https://www.gpui.rs/) and
[gpui-kit](https://gpui-kit.com/), driven from Python. A Rust host
owns the window, the event loop and the component tree; this client describes the UI as
operations on that tree and receives events. Standard library only.

```python
from gpui_bridge import App

app = App()
count = 0

root = app.div(class_="flex flex-col items-center gap-4 p-6 size-full")
label = app.text(content="0", class_="text-xl font-bold")

def add(_event=None):
    global count
    count += 1
    label.set(content=str(count))
    app.commit()

root.append(label, app.button(label="Add", variant="primary").on("click", add))
app.window(title="Counter", width=320, height=200, root=root)
app.commit()
app.run()
```

## Install

```sh
pip install gpui-bridge
```

Requires Python 3.10+. The wheel is built per platform and carries an optimized
`gpui-bridge-host` binary, so its tag names that platform (`py3-none-win_amd64`,
`py3-none-macosx_11_0_arm64`, `py3-none-manylinux_2_35_x86_64`) and no Rust toolchain is
needed at runtime. Those three are the platforms with a wheel; the host has been drawn in a real
window only on Windows so far.

`class` is a Python keyword: write `class_="..."`, and the client strips the trailing underscore.
Props and events are keyword arguments, and every node has `append`, `insert_before`, `set`,
`remove`, `on` and `off`.

## Host lookup

1. `$GPUI_BRIDGE_HOST`
2. `gpui_bridge/bin/gpui-bridge-host[.exe]` inside the installed package
3. A Cargo build in `target/debug` or `target/release`, searched upwards (a checkout)

## Links

- [Protocol specification](https://github.com/InDIOS/gpui-bridge/blob/main/docs/PROTOCOL.md)
  and the [conformance transcripts](https://github.com/InDIOS/gpui-bridge/tree/main/conformance):
  everything a client in another language needs.
- [Examples](https://github.com/InDIOS/gpui-bridge/tree/main/examples).
- Node.js and React clients: `gpui-bridge` and `gpui-bridge-react` on npm.

Protocol version 1. MIT licensed.
