package gpui_bridge

import (
	"os"
	"reflect"
	"runtime"
	"strings"
	"testing"
	"time"
)

func f64(v float64) *float64 { return &v }

// findHost resuelve el binario del host para las pruebas; salta si no está construido.
func findHost(t *testing.T) (string, bool) {
	t.Helper()
	path := defaultHostPath()
	if _, err := os.Stat(path); err == nil {
		return path, true
	}
	t.Logf("host no encontrado en %s", path)
	return "", false
}

func mustApp(t *testing.T) *App {
	t.Helper()
	hostPath, ok := findHost(t)
	if !ok {
		t.Skip("host gpui-bridge no disponible (compílalo con `cargo build -p gpui-bridge-host`)")
	}
	app, err := NewApp(AppOptions{HostPath: hostPath, Headless: true})
	if err != nil {
		t.Fatalf("NewApp: %v", err)
	}
	return app
}

func TestHandshakeCompletes(t *testing.T) {
	app := mustApp(t)
	exit := app.Close()
	if exit.Code == nil || *exit.Code != 0 {
		t.Fatalf("el host salió con código %v, se esperaba 0", exit.Code)
	}
}

func TestValidTreeCommitsWithoutErrors(t *testing.T) {
	app := mustApp(t)
	var errors []HostError
	app.OnError(func(err HostError) { errors = append(errors, err) })

	root := app.Div(Props{"class": "flex flex-col gap-2 p-4"})
	label := app.Text(Props{"content": "Clicks: 0"})
	button := app.Button(Props{"label": "Add", "variant": "primary"})
	input := app.Input(Props{"placeholder": "Type"})
	checkbox := app.Checkbox(Props{"label": "Done", "controlled": true})
	root.Append(label, button, input, checkbox)
	app.Window(WindowOptions{Title: "Test", Width: f64(400), Height: f64(300), Root: root, Theme: ThemeDark})
	app.Commit()

	label.Set(Props{"content": "Clicks: 1"})
	button.Set(Props{"variant": nil, "disabled": true}).Off("click")
	checkbox.Remove()
	app.Commit()

	exit := app.Close()
	if exit.Code == nil || *exit.Code != 0 {
		t.Fatalf("el host salió con código %v, se esperaba 0", exit.Code)
	}
	if len(errors) != 0 {
		t.Fatalf("errores inesperados: %#v", errors)
	}
}

func TestImageBase64InvalidProps(t *testing.T) {
	app := mustApp(t)
	var errors []HostError
	app.OnError(func(err HostError) { errors = append(errors, err) })

	// Un create rechazado no deja nodo, así que los datos malos llegan con setProps.
	image := app.Image(Props{"data": "aGVsbG8=", "fit": "contain"})
	app.Window(WindowOptions{Root: app.Div(Props{}).Append(image)})
	app.Commit()
	image.Set(Props{"data": "not base64!"})
	app.Commit()

	app.Close()
	if len(errors) != 1 || errors[0].Code != "invalid_props" || errors[0].Op != "setProps" {
		t.Fatalf("esperaba invalid_props en setProps, obtuve %#v", errors)
	}
}

func TestHostErrorsTypedAndHostKeepsRunning(t *testing.T) {
	app := mustApp(t)
	var errors []HostError
	app.OnError(func(err HostError) { errors = append(errors, err) })

	gone := app.Text(Props{"content": "gone"})
	gone.Remove()
	root := app.Div(Props{})
	root.Append(gone) // el host ya no conoce este nodo
	app.Button(Props{"label": "x"}).Set(Props{"disabled": "yes"})
	app.Commit()

	exit := app.Close()
	if exit.Code == nil || *exit.Code != 0 {
		t.Fatalf("el host salió con código %v, se esperaba 0", exit.Code)
	}
	if len(errors) != 2 || errors[0].Code != "unknown_node" || errors[1].Code != "invalid_props" {
		t.Fatalf("errores inesperados: %#v", errors)
	}
}

func TestEventsDispatched(t *testing.T) {
	app := mustApp(t)
	var seen []string
	btn := app.Button(Props{"label": "Go"}).On("click", func(payload any) {
		seen = append(seen, "click")
	})
	app.Commit()
	// Inyecta un evento del host como si viniera de la red.
	app.dispatch(map[string]any{"type": "event", "id": float64(btn.Id()), "event": "click"}) // el JSON real trae los números como float64
	if len(seen) != 1 || seen[0] != "click" {
		t.Fatalf("evento click no despachado: %v", seen)
	}
	app.Close()
}

func TestDefaultHostPath(t *testing.T) {
	exe := "gpui-bridge-host"
	if runtime.GOOS == "windows" {
		exe += ".exe"
	}
	saved, had := os.LookupEnv("GPUI_BRIDGE_HOST")
	os.Unsetenv("GPUI_BRIDGE_HOST")
	t.Cleanup(func() {
		if had {
			os.Setenv("GPUI_BRIDGE_HOST", saved)
		} else {
			os.Unsetenv("GPUI_BRIDGE_HOST")
		}
	})

	path := defaultHostPath()
	if _, err := os.Stat(path); err == nil {
		if !strings.Contains(path, "target") || !strings.HasSuffix(path, exe) {
			t.Fatalf("defaultHostPath no apunta a un target del host: %s", path)
		}
	} else {
		t.Skipf("host no construido en %s", path)
	}

	os.Setenv("GPUI_BRIDGE_HOST", "un-host-explicito")
	if got := defaultHostPath(); got != "un-host-explicito" {
		t.Fatalf("defaultHostPath ignoró GPUI_BRIDGE_HOST: %s", got)
	}
}

func TestMissingHostBinaryFails(t *testing.T) {
	_, err := NewApp(AppOptions{HostPath: "does-not-exist.exe", Headless: true})
	if err == nil {
		t.Fatal("esperaba un error por host faltante")
	}
	if !strings.Contains(err.Error(), "host no encontrado en does-not-exist.exe") {
		t.Fatalf("mensaje de error no útil: %v", err)
	}
}

func TestHandlersCanUseTheAPIWhileMainDoes(t *testing.T) {
	app := mustApp(t)
	clicked := make(chan struct{})
	label := app.Text(Props{"content": "0"})
	button := app.Button(Props{"label": "Go"})
	button.On("click", func(any) {
		label.Set(Props{"content": "1"})
		_ = app.Commit()
		close(clicked)
	})
	app.Window(WindowOptions{Root: app.Div(Props{}).Append(label, button)})
	if err := app.Commit(); err != nil {
		t.Fatal(err)
	}
	if err := app.Simulate(button.Id(), "click", nil, false); err != nil {
		t.Fatal(err)
	}
	// La goroutine principal usa la API mientras corre el handler, para que -race lo vea.
	timeout := time.After(5 * time.Second)
	for running := true; running; {
		select {
		case <-clicked:
			running = false
		case <-timeout:
			t.Fatal("el click simulado no llegó al handler")
		default:
			app.Div(nil)
		}
	}

	exit := app.Close()
	if again := app.Run(); !reflect.DeepEqual(again, exit) {
		t.Fatalf("Run tras Close devolvió %#v, se esperaba %#v", again, exit)
	}
}
