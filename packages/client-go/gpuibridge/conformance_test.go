package gpuibridge

import (
	"encoding/json"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"
)

// TestMessagesMatchConformance checks that what the client writes is a `>` line of
// conformance/ (PROTOCOL.md §9). The host already replays the transcripts; this tests the
// other side: that the client can produce those lines.
func TestMessagesMatchConformance(t *testing.T) {
	paths, _ := filepath.Glob(filepath.Join("..", "..", "..", "conformance", "*.txt"))
	if len(paths) == 0 {
		t.Fatal("no transcripts in the root conformance/")
	}
	sent := map[string]bool{}
	for _, path := range paths {
		data, err := os.ReadFile(path)
		if err != nil {
			t.Fatal(err)
		}
		for _, line := range strings.Split(string(data), "\n") {
			if rest, ok := strings.CutPrefix(strings.TrimRight(line, "\r"), "> "); ok {
				sent[rest] = true
			}
		}
	}

	// The window goes through App to cover the WindowOptions conversion.
	app := &App{tree: newTree(), nextWindowID: 2}
	app.CreateWindow(WindowOptions{Root: &Node{id: 3}})
	window := app.tree.pending[0]

	id := func(v NodeID) *NodeID { return &v }
	cases := []struct {
		got  ClientMessage
		want string
	}{
		{helloMsg(), `{"op":"hello","protocol":1}`},
		{commitMsg(), `{"op":"commit"}`},
		{window, `{"op":"window","id":2,"root":3}`},
		{closeWindowMsg(2), `{"op":"closeWindow","id":2}`},
		{createMsg(6, "Spinner", nil, nil), `{"op":"create","id":6,"type":"Spinner"}`},
		{createMsg(3, "Button", Props{"label": "Go"}, []string{"click"}), `{"op":"create","id":3,"type":"Button","props":{"label":"Go"},"events":["click"]}`},
		{createMsg(3, "Label", Props{"content": "Total", "secondary": "(12)", "highlight": "12"}, nil), `{"op":"create","id":3,"type":"Label","props":{"content":"Total","secondary":"(12)","highlight":"12"}}`},
		{createMsg(8, "Clipboard", Props{"value": "example.com", "tooltip": "Copy the address", "a11y": map[string]any{"label": "Copy address"}}, []string{"copied"}), `{"op":"create","id":8,"type":"Clipboard","props":{"value":"example.com","tooltip":"Copy the address","a11y":{"label":"Copy address"}},"events":["copied"]}`},
		{appendMsg(1, 2), `{"op":"append","parent":1,"child":2}`},
		{insertBeforeMsg(1, 3, 4), `{"op":"insertBefore","parent":1,"child":3,"before":4}`},
		{setPropsMsg(6, Props{"checked": true}), `{"op":"setProps","id":6,"props":{"checked":true}}`},
		{setPropsMsg(1, Props{"a11y": nil}), `{"op":"setProps","id":1,"props":{"a11y":null}}`},
		{setEventsMsg(2, []string{"click"}), `{"op":"setEvents","id":2,"events":["click"]}`},
		{setEventsMsg(1, []string{}), `{"op":"setEvents","id":1,"events":[]}`},
		{removeMsg(4), `{"op":"remove","id":4}`},
		{simulateMsg(id(1), "click", nil, false), `{"op":"simulate","id":1,"event":"click"}`},
		{simulateMsg(id(7), "submit", "buy milk", true), `{"op":"simulate","id":7,"event":"submit","value":"buy milk"}`},
		{simulateMsg(nil, "key", "f5", true), `{"op":"simulate","event":"key","value":"f5"}`},
	}
	for _, c := range cases {
		if !sent[c.want] {
			t.Errorf("line is not in conformance/: %s", c.want)
			continue
		}
		line, err := json.Marshal(c.got)
		if err != nil {
			t.Fatal(err)
		}
		var got, want any
		_ = json.Unmarshal(line, &got)
		_ = json.Unmarshal([]byte(c.want), &want)
		if !reflect.DeepEqual(got, want) {
			t.Errorf("the client writes %s, the transcript says %s", line, c.want)
		}
	}
}
