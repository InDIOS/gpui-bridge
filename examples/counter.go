// Counter: a label and two buttons, driven entirely from Go.
// Run: go run ./examples   (after building the host: cargo build -p gpui-bridge-host)
//
//	go run ./examples --debug  (shows the host's logs on stderr)
package main

import (
	"flag"
	"fmt"
	"log"
	"strconv"

	"github.com/InDIOS/gpui-bridge/packages/client-go/gpuibridge"
)

func main() {
	debug := flag.Bool("debug", false, "show the host's logs")
	flag.Parse()

	app, err := gpuibridge.NewApp(gpuibridge.AppOptions{Debug: *debug})
	if err != nil {
		log.Fatal(err)
	}

	count := 0
	root := app.Div(gpuibridge.Props{"class": "flex flex-col items-center justify-center gap-4 p-6 size-full bg-background"})
	label := app.Text(gpuibridge.Props{"content": "0", "class": "text-xl font-bold text-foreground"})
	buttons := app.Div(gpuibridge.Props{"class": "flex flex-row gap-2"})
	subtract := app.Button(gpuibridge.Props{"label": "Subtract", "variant": "secondary"})
	add := app.Button(gpuibridge.Props{"label": "Add", "variant": "primary"})

	update := func(delta int) {
		count += delta
		label.Set(gpuibridge.Props{"content": strconv.Itoa(count)})
		app.Commit()
	}

	subtract.On("click", func(_ any) { update(-1) })
	add.On("click", func(_ any) { update(1) })

	buttons.Append(subtract, add)
	root.Append(label, buttons)
	app.Window(gpuibridge.WindowOptions{Title: "Counter (Go)", Width: gpuibridge.Ptr(320.0), Height: gpuibridge.Ptr(200.0), Root: root})
	app.Commit()

	exit := app.Run()
	if exit.Code != nil && *exit.Code != 0 {
		fmt.Printf("the host exited with code %d\n", *exit.Code)
	}
}
