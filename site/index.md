---
layout: home

hero:
  name: gpui-bridge
  text: Native GPUI apps from any language
  tagline: A Rust host draws the window with GPUI and gpui-kit. Your program, in Python, Node.js, React, Go or anything that can start a process, describes the UI over stdin and stdout.
  image:
    src: https://github.com/user-attachments/assets/35c33e2e-2844-4091-98ca-5a3822cde259
    alt: gpui-bridge
  actions:
    - theme: brand
      text: How it works
      link: /guide/how-it-works
    - theme: alt
      text: Read the protocol
      link: /protocol
    - theme: alt
      text: GitHub
      link: https://github.com/InDIOS/gpui-bridge

features:
  - icon: 🪶
    title: No Rust toolchain
    details: The clients ship a prebuilt host for Windows x64, macOS arm64 and Linux x64. pip install or npm install, and run.
  - icon: 🔌
    title: One small protocol
    details: JSON Lines over stdio. Create nodes, set props, append children, commit, and receive events. That's the whole contract.
  - icon: 🧪
    title: Conformance built in
    details: Wire-level transcripts in conformance/ are the test the host and every client pass, so a new client knows exactly what to send.
  - icon: 🎨
    title: Themed out of the box
    details: Tailwind-like class strings that use the theme's colors. Light, dark and custom themes come for free.
  - icon: 🪟
    title: A real desktop app
    details: Several windows, frameless title bars, menus, shortcuts, tray icon, file dialogs, notifications and accessibility through AccessKit.
  - icon: 🧩
    title: Most of gpui-kit
    details: Forms, tables, virtual lists, trees, charts, docks, chat, command palettes and more, as node types.
---

## Pick your language

The same counter in each client. Each one is a few lines of code plus a host that comes with the
package.

::: code-group

```python [Python]
# pip install gpui-bridge
from gpui_bridge import App

app = App()
count = 0
label = app.text(content="0", class_="text-xl font-bold")

def add(_event=None):
    global count
    count += 1
    label.set(content=str(count))
    app.commit()

root = app.div(class_="flex flex-col items-center gap-4 p-6 size-full")
root.append(label, app.button(label="Add", variant="primary").on("click", add))
app.window(title="Counter", width=320, height=200, root=root)
app.commit()
app.run()
```

```ts [Node.js]
// npm install gpui-bridge
import { createApp } from "gpui-bridge";

const app = await createApp();
let count = 0;
const label = app.text({ content: "0", class: "text-xl font-bold" });
const add = app.button({ label: "Add", variant: "primary" }).on("click", () => {
  label.set({ content: String(++count) });
  app.commit();
});
app.window({
  title: "Counter", width: 320, height: 200,
  root: app.div({ class: "flex flex-col items-center gap-4 p-6 size-full" }).append(label, add),
});
app.commit();
```

```tsx [React]
// npm install gpui-bridge-react gpui-bridge react
import { useState } from "react";
import { Button, Div, Text, render } from "gpui-bridge-react";

function Counter() {
  const [count, setCount] = useState(0);
  return (
    <Div class="flex flex-col items-center gap-4 p-6 size-full">
      <Text class="text-xl font-bold">{String(count)}</Text>
      <Button variant="primary" onClick={() => setCount(count + 1)}>Add</Button>
    </Div>
  );
}

await render(<Counter />, { window: { title: "Counter", width: 320, height: 200 } });
```

```go [Go]
// go get github.com/InDIOS/gpui-bridge/packages/client-go@latest
package main

import (
	"strconv"

	"github.com/InDIOS/gpui-bridge/packages/client-go/gpuibridge"
)

func main() {
	app, err := gpuibridge.NewApp(gpuibridge.AppOptions{})
	if err != nil {
		panic(err)
	}
	count := 0
	root := app.Div(gpuibridge.Props{"class": "flex flex-col items-center gap-4 p-6 size-full"})
	label := app.Text(gpuibridge.Props{"content": "0", "class": "text-xl font-bold"})
	add := app.Button(gpuibridge.Props{"label": "Add", "variant": "primary"}).On("click", func(any) {
		count++
		label.Set(gpuibridge.Props{"content": strconv.Itoa(count)})
		app.Commit()
	})
	root.Append(label, add)
	app.Window(gpuibridge.WindowOptions{Title: "Counter", Width: gpuibridge.Ptr(320.0), Height: gpuibridge.Ptr(200.0), Root: root})
	app.Commit()
	app.Run()
}
```

:::

Install details, host lookup and platform notes for each one are under
[Clients](/clients/python). For what each component looks like and does, see
[gpui-kit](https://gpui-kit.com/) and [GPUI](https://www.gpui.rs/); this site covers how
gpui-bridge drives them.
