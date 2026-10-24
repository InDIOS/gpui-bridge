package gpui_bridge

import (
	"encoding/json"
	"fmt"
	"sync"
	"time"
)

// WindowOptions configura una ventana (PROTOCOL.md §3).
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

// ThemeSet es un fichero de tema de gpui-component (PROTOCOL.md §3).
type ThemeSet struct {
	Name   string        `json:"name,omitempty"`
	Themes []ThemeConfig `json:"themes"`
}

// ThemeConfig es una entrada de tema (PROTOCOL.md §3).
type ThemeConfig struct {
	Name       string            `json:"name,omitempty"`
	Mode       string            `json:"mode,omitempty"`
	FontSize   *float64          `json:"font_size,omitempty"`
	FontFamily string            `json:"font_family,omitempty"`
	Radius     *float64          `json:"radius,omitempty"`
	Colors     map[string]string `json:"colors,omitempty"`
}

// WindowEventPayload es un evento de ventana (PROTOCOL.md §3 y §6).
type WindowEventPayload struct {
	Id      WindowId
	Event   string
	X       *float64
	Y       *float64
	Width   *float64
	Height  *float64
	Visible *bool
}

// AppOptions configura la creación de un App.
type AppOptions struct {
	// HostPath es la ruta al binario del host. Vacío significa defaultHostPath().
	HostPath string
	// Debug reenvía los logs del host (stderr) a la consola.
	Debug bool
	// Headless arranca el host con --no-window (sin ventana real; para pruebas).
	Headless bool
}

// App es el cliente conectado a un host gpui-bridge.
type App struct {
	transport *Transport
	tree      *Tree
	hostPath  string

	mu   sync.Mutex
	done chan struct{} // se cierra cuando el host sale; exit queda fijo desde entonces
	exit HostExit

	nextWindowId WindowId

	// Handlers (invocados fuera de mu desde el bucle de lectura).
	onError        func(HostError)
	onWindowEvent  func(WindowEventPayload)
	onWindowClosed func(WindowId)
	onNotification func(id string, window WindowId)
	onShortcut     func(keys string, window WindowId)
	onTray         func(event, value string)
	onResult       func(request uint64, body map[string]any)
}

// NewApp arranca el host, hace el handshake y devuelve un App listo para usar.
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
		nextWindowId: 2, // la ventana 1 es la principal
	}

	// Handshake: el cliente envía primero y espera `ready`. Se hace de forma síncrona
	// y SOLO entonces se arranca el bucle de lectura, así nunca hay dos lectores
	// concurrentes sobre stdout (bufio.Reader no es seguro para eso).
	if err := tr.Send(helloMsg()); err != nil {
		tr.Wait()
		return nil, fmt.Errorf("no se pudo escribir al host gpui-bridge: %w", err)
	}
	line, err := tr.ReadLineTimeout(time.Duration(HANDSHAKE_TIMEOUT_MS) * time.Millisecond)
	if err != nil {
		tr.Close()
		tr.Wait()
		return nil, fmt.Errorf("el host gpui-bridge no respondió el handshake en %d ms: %w", HANDSHAKE_TIMEOUT_MS, err)
	}
	var ready map[string]any
	if err := json.Unmarshal([]byte(line), &ready); err != nil || ready["type"] != "ready" {
		tr.Close()
		tr.Wait()
		return nil, fmt.Errorf("handshake del host gpui-bridge falló: %s", line)
	}
	go app.readLoop()
	return app, nil
}

// Commit envía todas las operaciones encoladas y un `commit` al host. Devuelve el primer
// error de escritura (p. ej. el host ya salió); el resto del lote no se envía.
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

// Simulate juega una acción de usuario sobre un nodo. Solo funciona con --no-window
// y solo en fixtures (PROTOCOL.md §9); útil para pruebas sin ventana real.
func (a *App) Simulate(id NodeId, event string, value any, hasValue bool) error {
	a.mu.Lock()
	defer a.mu.Unlock()
	return a.transport.Send(simulateMsg(&id, event, value, hasValue))
}

// Close cierra stdin y espera a que el host termine, devolviendo su HostExit.
func (a *App) Close() HostExit {
	_ = a.transport.Close()
	<-a.done
	return a.exit
}

// Run bloquea la goroutine actual hasta que el host termine y devuelve su HostExit.
// Los handlers corren en otra goroutine; la API de App y Node es segura desde ellos.
func (a *App) Run() HostExit {
	<-a.done
	return a.exit
}

// readLoop lee líneas del host y las despacha. Termina (y publica el exit) cuando
// stdout se cierra (el host salió).
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
	// No hay handler de corrupt lines en la API pública por ahora; se ignora.
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

// OnError registra un handler para los errores tipados del host.
func (a *App) OnError(handler func(HostError)) {
	a.mu.Lock()
	a.onError = handler
	a.mu.Unlock()
}

// OnWindowEvent registra un handler para eventos de ventana.
func (a *App) OnWindowEvent(handler func(WindowEventPayload)) {
	a.mu.Lock()
	a.onWindowEvent = handler
	a.mu.Unlock()
}

// OnWindowClosed registra un handler para el cierre de ventanas.
func (a *App) OnWindowClosed(handler func(WindowId)) {
	a.mu.Lock()
	a.onWindowClosed = handler
	a.mu.Unlock()
}

// OnNotification registra un handler para notificaciones (toasts).
func (a *App) OnNotification(handler func(id string, window WindowId)) {
	a.mu.Lock()
	a.onNotification = handler
	a.mu.Unlock()
}

// OnShortcut registra un handler para atajos de teclado.
func (a *App) OnShortcut(handler func(keys string, window WindowId)) {
	a.mu.Lock()
	a.onShortcut = handler
	a.mu.Unlock()
}

// OnTray registra un handler para eventos del icono de la bandeja.
func (a *App) OnTray(handler func(event, value string)) {
	a.mu.Lock()
	a.onTray = handler
	a.mu.Unlock()
}

// OnResult registra un handler para respuestas de acciones del sistema.
func (a *App) OnResult(handler func(request uint64, body map[string]any)) {
	a.mu.Lock()
	a.onResult = handler
	a.mu.Unlock()
}

// --- helpers de extracción --------------------------------------------------

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

// payloadMap devuelve el cuerpo de un evento: el mensaje sin las claves type/id/event.
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
