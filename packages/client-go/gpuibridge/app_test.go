package gpuibridge

import (
	"os"
	"path/filepath"
	"reflect"
	"runtime"
	"strings"
	"sync"
	"testing"
	"time"
)

func f64(v float64) *float64 { return &v }

// findHost resolves the host binary for the tests; it skips if the host is not built.
func findHost(t *testing.T) (string, bool) {
	t.Helper()
	path := defaultHostPath()
	if _, err := os.Stat(path); err == nil {
		return path, true
	}
	t.Logf("host not found at %s", path)
	return "", false
}

func mustApp(t *testing.T) *App {
	t.Helper()
	hostPath, ok := findHost(t)
	if !ok {
		t.Skip("gpui-bridge host not available (build it with `cargo build -p gpui-bridge-host`)")
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
		t.Fatalf("the host exited with code %v, want 0", exit.Code)
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
		t.Fatalf("the host exited with code %v, want 0", exit.Code)
	}
	if len(errors) != 0 {
		t.Fatalf("errores inesperados: %#v", errors)
	}
}

func TestImageBase64InvalidProps(t *testing.T) {
	app := mustApp(t)
	var errors []HostError
	app.OnError(func(err HostError) { errors = append(errors, err) })

	// A rejected create leaves no node, so the bad data comes with setProps.
	image := app.Image(Props{"data": "aGVsbG8=", "fit": "contain"})
	app.Window(WindowOptions{Root: app.Div(Props{}).Append(image)})
	app.Commit()
	image.Set(Props{"data": "not base64!"})
	app.Commit()

	app.Close()
	if len(errors) != 1 || errors[0].Code != "invalid_props" || errors[0].Op != "setProps" {
		t.Fatalf("want invalid_props on setProps, got %#v", errors)
	}
}

func TestHostErrorsTypedAndHostKeepsRunning(t *testing.T) {
	app := mustApp(t)
	var errors []HostError
	app.OnError(func(err HostError) { errors = append(errors, err) })

	gone := app.Text(Props{"content": "gone"})
	gone.Remove()
	root := app.Div(Props{})
	root.Append(gone) // the host no longer knows this node
	app.Button(Props{"label": "x"}).Set(Props{"disabled": "yes"})
	app.Commit()

	exit := app.Close()
	if exit.Code == nil || *exit.Code != 0 {
		t.Fatalf("the host exited with code %v, want 0", exit.Code)
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
	// Inject a host event as if it came over the wire.
	app.dispatch(map[string]any{"type": "event", "id": float64(btn.ID()), "event": "click"}) // real JSON decodes numbers as float64
	if len(seen) != 1 || seen[0] != "click" {
		t.Fatalf("click event not dispatched: %v", seen)
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
			t.Fatalf("defaultHostPath does not point at a host target: %s", path)
		}
	} else {
		t.Skipf("host not built at %s", path)
	}

	os.Setenv("GPUI_BRIDGE_HOST", "an-explicit-host")
	if got := defaultHostPath(); got != "an-explicit-host" {
		t.Fatalf("defaultHostPath ignored GPUI_BRIDGE_HOST: %s", got)
	}
}

func TestFormControlsWithStateDispatchValues(t *testing.T) {
	app := mustApp(t)
	var (
		mu   sync.Mutex
		seen []any
	)
	add := func(payload any) {
		mu.Lock()
		seen = append(seen, payload.(map[string]any)["value"])
		mu.Unlock()
	}

	day := app.DatePicker(Props{"value": "2024-08-03", "placeholder": "Pick a day"}).On("change", add)
	time_ := app.TimeField(Props{"value": "09:30"}).On("change", add)
	color := app.ColorPicker(Props{"value": "#3366ff", "palette": []any{"#ff0000", "#00ff0080"}}).On("change", add)
	langs := app.Combobox(Props{
		"options": []any{
			map[string]any{"value": "rs", "label": "Rust"},
			map[string]any{"value": "go", "label": "Go"},
		},
		"value": []any{"rs"}, "placeholder": "Languages", "searchable": true,
	}).On("change", add)
	app.Window(WindowOptions{Root: app.Div(Props{}).Append(day, time_, color, langs)})
	if err := app.Commit(); err != nil {
		t.Fatal(err)
	}

	// `Simulate` puts real JSON on the wire, so the `value` of a `Combobox` change
	// arrives decoded as []interface{}: the array check that lets Combobox enter M14.
	steps := []struct {
		node  NodeID
		value any
	}{
		{day.ID(), "2024-08-10"},
		{day.ID(), nil}, // clearing the date
		{time_.ID(), "18:05"},
		{color.ID(), "#FF0000"},
		{color.ID(), nil}, // clearing the color
		{langs.ID(), []any{"rs", "go"}},
		{langs.ID(), []any{"go"}},
	}
	for _, s := range steps {
		if err := app.Simulate(s.node, "change", s.value, true); err != nil {
			t.Fatal(err)
		}
	}
	deadline := time.After(5 * time.Second)
	for {
		mu.Lock()
		n := len(seen)
		mu.Unlock()
		if n == len(steps) {
			break
		}
		select {
		case <-deadline:
			t.Fatalf("only %d of %d events arrived: %#v", n, len(steps), seen)
		case <-time.After(10 * time.Millisecond):
		}
	}

	app.Close()
	mu.Lock()
	defer mu.Unlock()
	want := []any{"2024-08-10", nil, "18:05", "#FF0000", nil, []any{"rs", "go"}, []any{"go"}}
	if !reflect.DeepEqual(seen, want) {
		t.Fatalf("got %#v, want %#v", seen, want)
	}
}

func TestChatNodesDispatchEvents(t *testing.T) {
	app := mustApp(t)
	var (
		mu   sync.Mutex
		seen []any
	)
	add := func(name string) func(any) {
		return func(payload any) {
			mu.Lock()
			seen = append(seen, []any{name, payload.(map[string]any)["value"]})
			mu.Unlock()
		}
	}

	file := app.Attachment(Props{"status": "failed", "title": "report.pdf"}).
		On("click", add("click")).On("remove", add("remove")).On("retry", add("retry"))
	quiz := app.Questionnaire(Props{"items": []any{
		map[string]any{"name": "plan", "label": "Plan", "choices": []any{map[string]any{"value": "free", "label": "Free"}}},
	}}).On("change", add("change")).On("submit", add("submit"))
	message := app.Message(Props{"alignment": "end", "avatarName": "Ada"}).Append(
		app.MessageHeader(Props{}).Append(app.Text(Props{"content": "Ada"})),
		app.Bubble(Props{"variant": "secondary"}).Append(app.Text(Props{"content": "Hi"})),
		app.MessageFooter(Props{}),
	)
	scroller := app.MessageScroller(Props{"jumpLabel": "Latest"}).
		Append(message, app.Marker(Props{"content": "Thinking", "loading": true}), file, quiz)
	app.Window(WindowOptions{Root: app.Div(Props{}).Append(scroller)})
	if err := app.Commit(); err != nil {
		t.Fatal(err)
	}

	steps := []struct {
		node     NodeID
		event    string
		value    any
		hasValue bool
	}{
		{file.ID(), "click", nil, false},
		{file.ID(), "remove", nil, false},
		{file.ID(), "retry", nil, false},
		{quiz.ID(), "change", map[string]any{"name": "plan", "choices": []any{"free"}}, true},
		{quiz.ID(), "submit", nil, false},
	}
	for _, s := range steps {
		if err := app.Simulate(s.node, s.event, s.value, s.hasValue); err != nil {
			t.Fatal(err)
		}
	}
	deadline := time.After(5 * time.Second)
	for {
		mu.Lock()
		n := len(seen)
		mu.Unlock()
		if n == len(steps) {
			break
		}
		select {
		case <-deadline:
			t.Fatalf("only %d of %d events arrived: %#v", n, len(steps), seen)
		case <-time.After(10 * time.Millisecond):
		}
	}

	app.Close()
	mu.Lock()
	defer mu.Unlock()
	answer := map[string]any{"name": "plan", "status": "answered", "choices": []any{"free"}, "text": nil}
	want := []any{
		[]any{"click", nil}, []any{"remove", nil}, []any{"retry", nil},
		[]any{"change", answer}, []any{"submit", []any{answer}},
	}
	if !reflect.DeepEqual(seen, want) {
		t.Fatalf("got %#v, want %#v", seen, want)
	}
}

func TestListDispatchesEvents(t *testing.T) {
	app := mustApp(t)
	var (
		mu   sync.Mutex
		seen []any
	)
	add := func(name string) func(any) {
		return func(payload any) {
			mu.Lock()
			seen = append(seen, []any{name, payload.(map[string]any)["value"]})
			mu.Unlock()
		}
	}

	list := app.List(Props{"value": "a", "searchable": true, "hasMore": true, "loadMoreThreshold": 5}).
		On("change", add("change")).On("confirm", add("confirm")).
		On("query", add("query")).On("loadMore", add("loadMore")).
		Append(
			app.ListItem(Props{"value": "a"}).Append(app.Text(Props{"content": "Apple"})),
			app.ListItem(Props{"value": "b"}).Append(app.Text(Props{"content": "Banana"})),
		)
	app.Window(WindowOptions{Root: app.Div(Props{}).Append(list)})
	if err := app.Commit(); err != nil {
		t.Fatal(err)
	}

	steps := []struct {
		event    string
		value    any
		hasValue bool
	}{
		{"confirm", "b", true},
		{"query", "ap", true},
		{"loadMore", nil, false},
	}
	for _, s := range steps {
		if err := app.Simulate(list.ID(), s.event, s.value, s.hasValue); err != nil {
			t.Fatal(err)
		}
	}
	// Confirming another row selects it first: one more event than steps.
	want := []any{
		[]any{"change", "b"}, []any{"confirm", "b"}, []any{"query", "ap"}, []any{"loadMore", nil},
	}
	deadline := time.After(5 * time.Second)
	for {
		mu.Lock()
		n := len(seen)
		mu.Unlock()
		if n == len(want) {
			break
		}
		select {
		case <-deadline:
			t.Fatalf("only %d of %d events arrived: %#v", n, len(want), seen)
		case <-time.After(10 * time.Millisecond):
		}
	}

	app.Close()
	mu.Lock()
	defer mu.Unlock()
	if !reflect.DeepEqual(seen, want) {
		t.Fatalf("got %#v, want %#v", seen, want)
	}
}

func TestMissingHostBinaryFails(t *testing.T) {
	_, err := NewApp(AppOptions{HostPath: "does-not-exist.exe", Headless: true})
	if err == nil {
		t.Fatal("want an error for a missing host")
	}
	if !strings.Contains(err.Error(), "host not found at does-not-exist.exe") {
		t.Fatalf("unhelpful error message: %v", err)
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
	if err := app.Simulate(button.ID(), "click", nil, false); err != nil {
		t.Fatal(err)
	}
	// The main goroutine uses the API while the handler runs, so -race can see it.
	timeout := time.After(5 * time.Second)
	for running := true; running; {
		select {
		case <-clicked:
			running = false
		case <-timeout:
			t.Fatal("the simulated click never reached the handler")
		default:
			app.Div(nil)
		}
	}

	exit := app.Close()
	if again := app.Run(); !reflect.DeepEqual(again, exit) {
		t.Fatalf("Run after Close returned %#v, want %#v", again, exit)
	}
}

func TestDefaultHostPathFindsTheHostOnPath(t *testing.T) {
	exe := "gpui-bridge-host"
	if runtime.GOOS == "windows" {
		exe += ".exe"
	}
	dir := t.TempDir()
	host := filepath.Join(dir, exe)
	if err := os.WriteFile(host, nil, 0o755); err != nil {
		t.Fatal(err)
	}
	t.Setenv("GPUI_BRIDGE_HOST", "")
	t.Setenv("PATH", dir)
	if got := defaultHostPath(); got != host {
		t.Fatalf("defaultHostPath = %s, want the host on PATH %s", got, host)
	}
}
