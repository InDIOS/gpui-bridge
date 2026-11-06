# Writing a new client

A client is any program that can start a process and read and write lines. The four clients in
the repository (Python, Node.js, React on top of Node, and Go) all follow the steps below. The
[Go client](https://github.com/InDIOS/gpui-bridge/tree/main/packages/client-go) is the smallest
reference. It uses only the standard library and fits in a handful of files.

The [protocol specification](/protocol) is normative. This page is the order to read it in and
the parts that are easy to get wrong.

## 1. Find the host

The existing clients search in this order and stop at the first hit:

1. An explicit path option (`hostPath`, `HostPath`…).
2. The `GPUI_BRIDGE_HOST` environment variable.
3. The binary your package ships. That's the npm platform package, the folder inside the Python
   wheel, or, for Go, a `gpui-bridge-host[.exe]` next to the executable or on `PATH`.
4. A Cargo build in `target/debug` or `target/release`, searched upwards. This makes a checkout of the repository work with no setup.

Keep 1 and 2 at least. They are how users and the repository's `pnpm dev` hand a host to your
client.

## 2. Start it and drain both pipes

Spawn the host with piped stdin, stdout and stderr.

- **stdout is the protocol.** Every line is one JSON message with a `type`. A line that is not
  valid JSON is a host bug. Log it and keep reading.
- **stderr is logs.** Read it continuously, or the pipe fills up and the host blocks. Keep the
  last lines. The Python client attaches them to the error it raises when the host crashes. Print
  them in a debug mode.
- **Exit.** The host exits when its stdin reaches EOF, so closing stdin is how a client shuts it
  down. It also exits after the user closes the last window, unless a tray icon is set. Your
  event loop ends when the process does. Report a non-zero exit code as a crash.

## 3. Handshake

Send `{"op":"hello","protocol":1}` first and wait for `{"type":"ready","protocol":1}`. If the
answer is an `error` with code `protocol_mismatch`, the host exits. Surface the versions to the
user. Until `ready`, the host ignores anything else you send.

## 4. Model the tree on your side

- **Allocate ids yourself.** Use a counter that starts at 1 and never reuses a value, even after
  `remove`. Id 0 is invalid. Window ids work the same way, and window 1 is the main window.
- **Queue operations, flush on `commit`.** Nothing is drawn until `commit`. The existing clients
  buffer messages and write them all when the user calls `commit()`. Write each message as one
  line ending in `\n`, and flush.
- **Send only what changes.** `setProps` merges, and `null` removes a prop.
- **Mirror the parent links.** `append` of a node that already has a parent moves it, and
  `remove` drops the whole subtree. If your client keeps node objects, update the same
  relationships so handlers of removed nodes can be released.

## 5. Dispatch events

- **Events are opt-in.** Send `"events":[...]` on `create`, or `setEvents` later (it replaces the
  set). A typical API sends `setEvents` when the user adds or removes a handler.
- **Route by `type`:** `event` (by node `id`), `windowClosed`, `windowEvent`, `tray`,
  `shortcut`, `notification`, `result` (the answer to a request such as `openDialog`, matched
  by the `request` number you chose) and `error`.
- **Errors are not fatal.** Match on `code` and `op`, never on `message`. Without a user
  handler, print them to stderr and keep going.
- **Ignore unknown fields and unknown message types.** A newer host may add some.
- **Threads.** If you read stdout on a separate thread or goroutine, decide where handlers run
  and say so in your docs. The Go client runs them on the reader goroutine and makes `App` and
  `Node` safe to call from there.

## 6. Ship the host with your package

Users should not need Rust. The host is built once per platform in CI and published as assets
of each [GitHub release](https://github.com/InDIOS/gpui-bridge/releases):
`gpui-bridge-host-win32-x64.exe`, `gpui-bridge-host-darwin-arm64` and
`gpui-bridge-host-linux-x64`, with `SHA256SUMS`. Pick the host whose version matches your
client's, and package it the way your ecosystem expects:

| Client | How the host travels |
|---|---|
| Node.js | One optional npm dependency per platform (`@indios/gpui-bridge-<platform>-<arch>`) |
| Python | Inside a platform-tagged wheel |
| Go | Downloaded from the release by the user (Go modules are source only) |

On macOS and Linux the binary needs the executable bit. On macOS it also needs the quarantine
attribute removed, because the host is not signed yet.

## 7. Test against the conformance transcripts

[`conformance/`](https://github.com/InDIOS/gpui-bridge/tree/main/conformance) holds wire-level
transcripts. Each `.txt` file is one session against `gpui-bridge-host --no-window`:

```text
# comment
> {"op":"hello","protocol":1}
< {"type":"ready","protocol":1}
```

The host replays them in its own tests. A client checks the other side: **every message it can
write must appear among the `>` lines**, field order aside. The Go client's
[`conformance_test.go`](https://github.com/InDIOS/gpui-bridge/blob/main/packages/client-go/gpuibridge/conformance_test.go)
collects all `>` lines and compares what each builder produces against them. Copy that test
first.

For end-to-end tests, run the real host with `--no-window`. It speaks the same protocol without
opening a window and dumps the tree to stderr after each commit. In that mode only, it also
accepts `simulate`, which plays a user action (a click, typed text, a menu choice) so you can
test your event dispatch without a screen. See [Protocol §9](/protocol#_9-conformance-fixtures).

## 8. Before you call it done

- [ ] `hello` and `ready`, with `protocol_mismatch` reported clearly.
- [ ] Ids from 1 up, never reused.
- [ ] Operations buffered until `commit`, one JSON object per line.
- [ ] stderr drained, with its tail kept for crash reports.
- [ ] Events subscribed per node and dispatched by `type`, with unknown fields ignored.
- [ ] `error` messages routed by `code`, never by `message`.
- [ ] Host lookup: explicit path, then `GPUI_BRIDGE_HOST`, then your packaged binary.
- [ ] Every message your client writes is a `>` line of `conformance/`.
- [ ] A counter example drawn and clicked in a real window.

Want it in the repository? Read [Contributing](/project/contributing). A protocol change has to
update the host, every client, the specification and the transcripts in the same commit.
