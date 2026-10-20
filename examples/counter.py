# Counter: a text and two buttons, driven entirely from Python (stdlib only).
# Run: python examples/counter.py   (after `cargo build` in host/; --debug shows host logs)

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "packages" / "client-python"))
from gpui_bridge import App

app = App(debug="--debug" in sys.argv)

count = 0

root = app.div(class_="flex flex-col items-center justify-center gap-4 p-6 size-full bg-background")
label = app.text(content="0", class_="text-xl font-bold text-foreground")
buttons = app.div(class_="flex flex-row gap-2")
subtract = app.button(label="Subtract", variant="secondary")
add = app.button(label="Add", variant="primary")


def update(delta):
    global count
    count += delta
    label.set(content=str(count))
    app.commit()


subtract.on("click", lambda: update(-1))
add.on("click", lambda: update(1))

buttons.append(subtract, add)
root.append(label, buttons)
app.window(title="Counter (Python)", width=320, height=200, root=root)
app.commit()
app.run()
