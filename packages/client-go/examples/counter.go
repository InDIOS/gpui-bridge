// Counter: un texto y dos botones, impulsado enteramente desde Go.
// Run: go run examples/counter.go   (tras compilar el host: cargo build en host/)
//
//	go run examples/counter.go --debug  (muestra los logs del host en stderr)
package main

import (
	"flag"
	"fmt"
	"log"
	"strconv"

	gpui "github.com/InDIOS/gpui-bridge/packages/client-go/gpui_bridge"
)

func main() {
	debug := flag.Bool("debug", false, "muestra los logs del host")
	flag.Parse()

	app, err := gpui.NewApp(gpui.AppOptions{Debug: *debug})
	if err != nil {
		log.Fatal(err)
	}

	count := 0
	root := app.Div(gpui.Props{"class": "flex flex-col items-center justify-center gap-4 p-6 size-full bg-background"})
	label := app.Text(gpui.Props{"content": "0", "class": "text-xl font-bold text-foreground"})
	buttons := app.Div(gpui.Props{"class": "flex flex-row gap-2"})
	subtract := app.Button(gpui.Props{"label": "Subtract", "variant": "secondary"})
	add := app.Button(gpui.Props{"label": "Add", "variant": "primary"})

	update := func(delta int) {
		count += delta
		label.Set(gpui.Props{"content": strconv.Itoa(count)})
		app.Commit()
	}

	subtract.On("click", func(_ any) { update(-1) })
	add.On("click", func(_ any) { update(1) })

	buttons.Append(subtract, add)
	root.Append(label, buttons)
	app.Window(gpui.WindowOptions{Title: "Counter (Go)", Width: gpui.Ptr(320.0), Height: gpui.Ptr(200.0), Root: root})
	app.Commit()

	exit := app.Run()
	if exit.Code != nil && *exit.Code != 0 {
		fmt.Printf("el host salió con código %d\n", *exit.Code)
	}
}
