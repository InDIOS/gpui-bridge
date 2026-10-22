# gpui-bridge (Python)

Native desktop UIs with [GPUI](https://www.gpui.rs/) and
[gpui-component](https://github.com/longbridge/gpui-component), driven from Python. A Rust host
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
`py3-none-macosx_11_0_arm64`, `py3-none-manylinux2014_x86_64`, …) and no Rust toolchain is
needed at runtime.

`class` is a Python keyword: write `class_="..."`, and the client strips the trailing underscore.
Props and events are keyword arguments, and every node has `append`, `insert_before`, `set`,
`remove`, `on` and `off`.

## Host lookup

1. `$GPUI_BRIDGE_HOST`
2. `gpui_bridge/bin/gpui-bridge-host[.exe]` inside the installed package
3. A Cargo build in `target/debug` or `target/release`, searched upwards (a checkout)

## Links

- Protocol specification and examples in the project repository (`docs/PROTOCOL.md`).
- Node.js and React clients: `gpui-bridge` and `gpui-bridge-react` on npm.

Protocol version 1. MIT licensed.
