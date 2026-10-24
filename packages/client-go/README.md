# gpui-bridge (Go)

Native desktop UIs with [GPUI](https://www.gpui.rs/) and
[gpui-kit](https://gpui-kit.com/), driven from Go. A Rust host
owns the window, the event loop and the component tree; this client describes the UI as
operations on that tree and receives events. Standard library only.

```go
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

## Install

```sh
go get github.com/InDIOS/gpui-bridge/packages/client-go@latest
```

Requires Go 1.22+. Go modules are source only, so the `gpui-bridge-host` binary does not come
with the module: download the one for your platform from the
[GitHub release](https://github.com/InDIOS/gpui-bridge/releases) of the same version
(`gpui-bridge-host-win32-x64.exe`, `gpui-bridge-host-darwin-arm64`,
`gpui-bridge-host-linux-x64`), rename it to `gpui-bridge-host[.exe]` and put it next to your
program or on `PATH`. On macOS and Linux, `chmod +x` it; on macOS, also
`xattr -d com.apple.quarantine gpui-bridge-host`, because the host is not signed yet.
`SHA256SUMS` in the release lists the checksums. Those three are the platforms with a host; it
has been drawn in a real window only on Windows so far.

Props and events are maps and callbacks, and every node has `Append`, `InsertBefore`, `Set`,
`Remove`, `On` and `Off`. Operations are queued and sent by `Commit`, which returns the first
write error (for example, when the host has exited). Event handlers run on the goroutine that
reads from the host; the `App` and `Node` methods are safe to call from them.

## Host lookup

1. `AppOptions.HostPath`
2. `$GPUI_BRIDGE_HOST`
3. `gpui-bridge-host[.exe]` next to the running executable
4. `gpui-bridge-host[.exe]` on `PATH`
5. A Cargo build in `target/debug` or `target/release`, searched upwards (a checkout)

## Links

- [Protocol specification](https://github.com/InDIOS/gpui-bridge/blob/main/docs/PROTOCOL.md)
  and the [conformance transcripts](https://github.com/InDIOS/gpui-bridge/tree/main/conformance):
  everything a client in another language needs.
- [Examples](https://github.com/InDIOS/gpui-bridge/tree/main/examples): `go run ./examples`
  from a checkout.
- Python client: `gpui-bridge` on PyPI. Node.js and React clients: `gpui-bridge` and
  `gpui-bridge-react` on npm.

Protocol version 1. MIT licensed.
