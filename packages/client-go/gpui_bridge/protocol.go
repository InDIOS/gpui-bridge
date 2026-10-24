// Package gpui_bridge is the Go client for the native gpui-bridge host.
//
// It speaks the language-independent JSON Lines protocol documented in
// docs/PROTOCOL.md and uses only the Go standard library (encoding/json, os/exec).
// Starting a process and reading/writing lines is enough: there is no need to read the
// host or any other client.
package gpui_bridge

import (
	"encoding/json"
)

// PROTOCOL_VERSION is the wire protocol version the host speaks (PROTOCOL.md §2).
const PROTOCOL_VERSION = 1

// HANDSHAKE_TIMEOUT_MS is how long to wait for the host's `ready` answer.
const HANDSHAKE_TIMEOUT_MS = 10_000

// NodeId is a client-assigned node id: positive, unique per session and never reused
// (PROTOCOL.md §5).
type NodeId = uint64

// WindowId is a client-assigned window id; window 1 is the main window and never needs
// an `id` (PROTOCOL.md §3).
type WindowId = uint64

// Props are a node's props on the wire. A `null` value removes the prop (PROTOCOL.md §5).
// In Go `class` is an ordinary map key.
type Props map[string]any

// WindowTheme is the gpui-component theme mode (PROTOCOL.md §3).
type WindowTheme = string

const (
	ThemeLight WindowTheme = "light"
	ThemeDark  WindowTheme = "dark"
)

// ClientMessage is a client → host message (key `op`).
type ClientMessage map[string]any

// --- client → host message constructors -------------------------------------

func helloMsg() ClientMessage {
	return ClientMessage{"op": "hello", "protocol": PROTOCOL_VERSION}
}

func commitMsg() ClientMessage {
	return ClientMessage{"op": "commit"}
}

func windowMsg(id WindowId, title string, width, height, x, y, minWidth, minHeight *float64,
	root *NodeId, theme string, themeSet *ThemeSet, frameless, minimized *bool, events []string) ClientMessage {

	m := ClientMessage{"op": "window", "id": id}
	if title != "" {
		m["title"] = title
	}
	if width != nil {
		m["width"] = *width
	}
	if height != nil {
		m["height"] = *height
	}
	if x != nil {
		m["x"] = *x
	}
	if y != nil {
		m["y"] = *y
	}
	if minWidth != nil {
		m["minWidth"] = *minWidth
	}
	if minHeight != nil {
		m["minHeight"] = *minHeight
	}
	if root != nil {
		m["root"] = *root
	}
	if theme != "" {
		m["theme"] = theme
	}
	if themeSet != nil {
		m["themeSet"] = themeSet
	}
	if frameless != nil {
		m["frameless"] = *frameless
	}
	if minimized != nil {
		m["minimized"] = *minimized
	}
	if events != nil {
		m["events"] = events
	}
	return m
}

func createMsg(id NodeId, typ string, props Props, events []string) ClientMessage {
	m := ClientMessage{"op": "create", "id": id, "type": typ}
	if props != nil {
		m["props"] = props
	}
	if events != nil {
		m["events"] = events
	}
	return m
}

func appendMsg(parent, child NodeId) ClientMessage {
	return ClientMessage{"op": "append", "parent": parent, "child": child}
}

func insertBeforeMsg(parent, child, before NodeId) ClientMessage {
	return ClientMessage{"op": "insertBefore", "parent": parent, "child": child, "before": before}
}

func setPropsMsg(id NodeId, props Props) ClientMessage {
	return ClientMessage{"op": "setProps", "id": id, "props": props}
}

func setEventsMsg(id NodeId, events []string) ClientMessage {
	return ClientMessage{"op": "setEvents", "id": id, "events": events}
}

func removeMsg(id NodeId) ClientMessage {
	return ClientMessage{"op": "remove", "id": id}
}

func closeWindowMsg(id WindowId) ClientMessage {
	return ClientMessage{"op": "closeWindow", "id": id}
}

func focusMsg(id NodeId) ClientMessage {
	return ClientMessage{"op": "focus", "id": id}
}

func trayMsg(icon, tooltip *string, menu any) ClientMessage {
	m := ClientMessage{"op": "tray"}
	if icon != nil {
		m["icon"] = *icon
	}
	if tooltip != nil {
		m["tooltip"] = *tooltip
	}
	if menu != nil {
		m["menu"] = menu
	}
	return m
}

func removeTrayMsg() ClientMessage {
	return ClientMessage{"op": "removeTray"}
}

func shortcutsMsg(keys []string) ClientMessage {
	return ClientMessage{"op": "shortcuts", "keys": keys}
}

func notifyMsg(window *WindowId, id, message, typ, title string, autohide, system *bool) ClientMessage {
	m := ClientMessage{"op": "notify", "message": message}
	if window != nil {
		m["window"] = *window
	}
	if id != "" {
		m["id"] = id
	}
	if typ != "" {
		m["type"] = typ
	}
	if title != "" {
		m["title"] = title
	}
	if autohide != nil {
		m["autohide"] = *autohide
	}
	if system != nil {
		m["system"] = *system
	}
	return m
}

func openDialogMsg(request uint64, files, directories, multiple *bool, prompt string) ClientMessage {
	m := ClientMessage{"op": "openDialog", "request": request}
	if files != nil {
		m["files"] = *files
	}
	if directories != nil {
		m["directories"] = *directories
	}
	if multiple != nil {
		m["multiple"] = *multiple
	}
	if prompt != "" {
		m["prompt"] = prompt
	}
	return m
}

func saveDialogMsg(request uint64, directory, name string) ClientMessage {
	m := ClientMessage{"op": "saveDialog", "request": request}
	if directory != "" {
		m["directory"] = directory
	}
	if name != "" {
		m["name"] = name
	}
	return m
}

func readClipboardMsg(request uint64) ClientMessage {
	return ClientMessage{"op": "readClipboard", "request": request}
}

func writeClipboardMsg(text string) ClientMessage {
	return ClientMessage{"op": "writeClipboard", "text": text}
}

func openUrlMsg(url string) ClientMessage {
	return ClientMessage{"op": "openUrl", "url": url}
}

func openPathMsg(path string) ClientMessage {
	return ClientMessage{"op": "openPath", "path": path}
}

// simulate plays a user action. It only works with --no-window and only in fixtures
// (PROTOCOL.md §9). `value` is included even when nil if hasValue is true
// (e.g. `"value":null`).
func simulateMsg(id *NodeId, event string, value any, hasValue bool) ClientMessage {
	m := ClientMessage{"op": "simulate", "event": event}
	if id != nil {
		m["id"] = *id
	}
	if hasValue {
		m["value"] = value
	}
	return m
}

// jsonLine serializes a client message as one compact JSON line.
func jsonLine(m ClientMessage) ([]byte, error) {
	return json.Marshal(m)
}

// Ptr returns a pointer to v; handy for the optional *float64 / *bool fields of
// WindowOptions and Props without helper variables.
func Ptr[T any](v T) *T {
	return &v
}
