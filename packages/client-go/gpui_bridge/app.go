package gpui_bridge

import (
	"encoding/json"
	"fmt"
	"sync"
	"time"
)

// WindowOptions configures a window (PROTOCOL.md §3).
type WindowOptions struct {
	Title     string
	Width     *float64
	Height    *float64
	X         *float64
	Y         *float64
	MinWidth  *float64
	MinHeight *float64
	Root      *Node
	Theme     string
	ThemeSet  *ThemeSet
	Frameless *bool
	Minimized *bool
	Events    []string
}

// ThemeSet is a gpui-component theme file (PROTOCOL.md §3).
type ThemeSet struct {
	Name   string        `json:"name,omitempty"`
	Themes []ThemeConfig `json:"themes"`
}

// ThemeConfig is one theme entry (PROTOCOL.md §3).
type ThemeConfig struct {
	Name       string            `json:"name,omitempty"`
	Mode       string            `json:"mode,omitempty"`
	FontSize   *float64          `json:"font_size,omitempty"`
	FontFamily string            `json:"font_family,omitempty"`
	Radius     *float64          `json:"radius,omitempty"`
	Colors     map[string]string `json:"colors,omitempty"`
}

// WindowEventPayload is a window event (PROTOCOL.md §3 and §6).
type WindowEventPayload struct {
	Id      WindowId
	Event   string
	X       *float64
	Y       *float64
	Width   *float64
	Height  *float64
	Visible *bool
}

// AppOptions configures how an App is created.
type AppOptions struct {
	// HostPath is the path to the host binary. Empty means defaultHostPath().
	HostPath string
	// Debug forwards the host's logs (stderr) to the console.
	Debug bool
	// Headless starts the host with --no-window (no real window; for tests).
	Headless bool
}

// App is the client connected to a gpui-bridge host.
type App struct {
	transport *Transport
	tree      *Tree
	hostPath  string

	mu   sync.Mutex
	done chan struct{} // closed when the host exits; exit is fixed from then on
	exit HostExit

	nextWindowId WindowId

	// Handlers (called outside mu from the read loop).
	onError        func(HostError)
	onWindowEvent  func(WindowEventPayload)
	onWindowClosed func(WindowId)
	onNotification func(id string, window WindowId)
	onShortcut     func(keys string, window WindowId)
	onTray         func(event, value string)
	onResult       func(request uint64, body map[string]any)
}

// NewApp starts the host, performs the handshake and returns an App ready to use.
func NewApp(opts AppOptions) (*App, error) {
	path := opts.HostPath
	if path == "" {
		path = defaultHostPath()
	}
	args := []string{}
	if opts.Headless {
		args = append(args, "--no-window")
	}
	tr, err := Spawn(path, opts.Debug, args...)
	if err != nil {
		return nil, err
	}

	app := &App{
		transport:    tr,
		tree:         newTree(),
		hostPath:     path,
		done:         make(chan struct{}),
		nextWindowId: 2, // window 1 is the main window
	}

	// Handshake: the client sends first and waits for `ready`. It runs synchronously and
	// ONLY then starts the read loop, so stdout never has two concurrent readers
	// (bufio.Reader is not safe for that).
	if err := tr.Send(helloMsg()); err != nil {
		tr.Wait()
		return nil, fmt.Errorf("could not write to the gpui-bridge host: %w", err)
	}
	line, err := tr.ReadLineTimeout(time.Duration(HANDSHAKE_TIMEOUT_MS) * time.Millisecond)
	if err != nil {
		tr.Close()
		tr.Wait()
		return nil, fmt.Errorf("the gpui-bridge host did not answer the handshake within %d ms: %w", HANDSHAKE_TIMEOUT_MS, err)
	}
	var ready map[string]any
	if err := json.Unmarshal([]byte(line), &ready); err != nil || ready["type"] != "ready" {
		tr.Close()
		tr.Wait()
		return nil, fmt.Errorf("gpui-bridge host handshake failed: %s", line)
	}
	go app.readLoop()
	return app, nil
}

// Commit sends every queued operation and a `commit` to the host. It returns the first
// write error (e.g. the host already exited); the rest of the batch is not sent.
func (a *App) Commit() error {
	a.mu.Lock()
	defer a.mu.Unlock()
	var err error
	a.tree.commit(func(m ClientMessage) {
		if err == nil {
			err = a.transport.Send(m)
		}
	})
	return err
}

// Simulate plays a user action on a node. It only works with --no-window and only in
// fixtures (PROTOCOL.md §9); useful for tests without a real window.
func (a *App) Simulate(id NodeId, event string, value any, hasValue bool) error {
	a.mu.Lock()
	defer a.mu.Unlock()
	return a.transport.Send(simulateMsg(&id, event, value, hasValue))
}

// Close closes stdin, waits for the host to exit and returns its HostExit.
func (a *App) Close() HostExit {
	_ = a.transport.Close()
	<-a.done
	return a.exit
}

// Run blocks the current goroutine until the host exits and returns its HostExit.
// Handlers run on another goroutine; the App and Node API is safe to call from them.
func (a *App) Run() HostExit {
	<-a.done
	return a.exit
}

// readLoop reads the host's lines and dispatches them. It ends (and publishes the exit)
// when stdout closes (the host exited).
func (a *App) readLoop() {
	for {
		line, err := a.transport.ReadLine()
		if err != nil {
			a.exit = a.transport.Wait()
			close(a.done)
			return
		}
		msg, err := parseHostMessage(line)
		if err != nil {
			a.dispatchCorrupt(line)
			continue
		}
		a.dispatch(msg)
	}
}

func (a *App) dispatchCorrupt(line string) {
	// The public API has no corrupt-line handler yet; ignore it.
	_ = line
}

func (a *App) dispatch(msg map[string]any) {
	typ, _ := msg["type"].(string)
	switch typ {
	case "error":
		a.handleError(msg)
	case "event":
		a.handleEvent(msg)
	case "windowClosed":
		a.handleWindowClosed(msg)
	case "windowEvent":
		a.handleWindowEvent(msg)
	case "notification":
		a.handleNotification(msg)
	case "shortcut":
		a.handleShortcut(msg)
	case "tray":
		a.handleTray(msg)
	case "result":
		a.handleResult(msg)
	}
}

func (a *App) handleError(msg map[string]any) {
	err := HostError{
		Code:    str(msg, "code"),
		Message: str(msg, "message"),
		Op:      str(msg, "op"),
	}
	a.mu.Lock()
	handler := a.onError
	a.mu.Unlock()
	if handler != nil {
		handler(err)
	}
}

func (a *App) handleEvent(msg map[string]any) {
	id := uint64(num(msg, "id"))
	event := str(msg, "event")
	payload := payloadMap(msg)
	a.mu.Lock()
	handler, ok := a.tree.handler(id, event)
	a.mu.Unlock()
	if ok && handler != nil {
		handler(payload)
	}
}

func (a *App) handleWindowClosed(msg map[string]any) {
	id := WindowId(num(msg, "id"))
	a.mu.Lock()
	handler := a.onWindowClosed
	a.mu.Unlock()
	if handler != nil {
		handler(id)
	}
}

func (a *App) handleWindowEvent(msg map[string]any) {
	p := WindowEventPayload{
		Id:      WindowId(num(msg, "id")),
		Event:   str(msg, "event"),
		X:       f64ptr(msg, "x"),
		Y:       f64ptr(msg, "y"),
		Width:   f64ptr(msg, "width"),
		Height:  f64ptr(msg, "height"),
		Visible: bptr(msg, "visible"),
	}
	a.mu.Lock()
	handler := a.onWindowEvent
	a.mu.Unlock()
	if handler != nil {
		handler(p)
	}
}

func (a *App) handleNotification(msg map[string]any) {
	a.mu.Lock()
	handler := a.onNotification
	a.mu.Unlock()
	if handler != nil {
		handler(str(msg, "id"), WindowId(num(msg, "window")))
	}
}

func (a *App) handleShortcut(msg map[string]any) {
	a.mu.Lock()
	handler := a.onShortcut
	a.mu.Unlock()
	if handler != nil {
		handler(str(msg, "keys"), WindowId(num(msg, "window")))
	}
}

func (a *App) handleTray(msg map[string]any) {
	a.mu.Lock()
	handler := a.onTray
	a.mu.Unlock()
	if handler != nil {
		handler(str(msg, "event"), str(msg, "value"))
	}
}

func (a *App) handleResult(msg map[string]any) {
	body := map[string]any{}
	for k, v := range msg {
		if k != "type" && k != "request" {
			body[k] = v
		}
	}
	a.mu.Lock()
	handler := a.onResult
	a.mu.Unlock()
	if handler != nil {
		handler(uint64(num(msg, "request")), body)
	}
}

// OnError registers a handler for the host's typed errors.
func (a *App) OnError(handler func(HostError)) {
	a.mu.Lock()
	a.onError = handler
	a.mu.Unlock()
}

// OnWindowEvent registers a handler for window events.
func (a *App) OnWindowEvent(handler func(WindowEventPayload)) {
	a.mu.Lock()
	a.onWindowEvent = handler
	a.mu.Unlock()
}

// OnWindowClosed registers a handler for closed windows.
func (a *App) OnWindowClosed(handler func(WindowId)) {
	a.mu.Lock()
	a.onWindowClosed = handler
	a.mu.Unlock()
}

// OnNotification registers a handler for notifications (toasts).
func (a *App) OnNotification(handler func(id string, window WindowId)) {
	a.mu.Lock()
	a.onNotification = handler
	a.mu.Unlock()
}

// OnShortcut registers a handler for keyboard shortcuts.
func (a *App) OnShortcut(handler func(keys string, window WindowId)) {
	a.mu.Lock()
	a.onShortcut = handler
	a.mu.Unlock()
}

// OnTray registers a handler for tray icon events.
func (a *App) OnTray(handler func(event, value string)) {
	a.mu.Lock()
	a.onTray = handler
	a.mu.Unlock()
}

// OnResult registers a handler for system action results.
func (a *App) OnResult(handler func(request uint64, body map[string]any)) {
	a.mu.Lock()
	a.onResult = handler
	a.mu.Unlock()
}

// --- extraction helpers -----------------------------------------------------

func str(m map[string]any, k string) string {
	v, _ := m[k].(string)
	return v
}

func num(m map[string]any, k string) float64 {
	v, _ := m[k].(float64)
	return v
}

func f64ptr(m map[string]any, k string) *float64 {
	v, ok := m[k].(float64)
	if !ok {
		return nil
	}
	return &v
}

func bptr(m map[string]any, k string) *bool {
	v, ok := m[k].(bool)
	if !ok {
		return nil
	}
	return &v
}

// payloadMap returns an event's body: the message without the type/id/event keys.
func payloadMap(msg map[string]any) map[string]any {
	p := make(map[string]any)
	for k, v := range msg {
		if k == "type" || k == "id" || k == "event" {
			continue
		}
		p[k] = v
	}
	return p
}
