# How it works

gpui-bridge splits a desktop app into two processes. The **host** is a Rust program
(`gpui-bridge-host`) that owns the window, the event loop, the component tree and rendering,
using [GPUI](https://www.gpui.rs/) and [gpui-kit](https://gpui-kit.com/). The **client** is your
program. It starts the host as a child process and describes the UI as operations on a tree.

<div class="arch">
  <div class="arch-box">
    <strong>Your app</strong>
    <span>Python, Node.js, React, Go, txiki.js…</span>
  </div>
  <div class="arch-wire">
    <span>operations →</span>
    <code>JSON Lines over stdin / stdout</code>
    <span>← events</span>
  </div>
  <div class="arch-box">
    <strong>gpui-bridge-host</strong>
    <span>Rust · GPUI · gpui-kit</span>
  </div>
</div>

Your code never links against Rust. It only writes and reads lines, so a client is a small
program in any language. The [protocol](/protocol) is the whole contract.

## A session on the wire

A counter session looks like this (the ids and the order depend on the client). `>` lines go from the client to the host, `<` lines
come back. The format is the one the [conformance transcripts](/guide/new-client#_7-test-against-the-conformance-transcripts) use.

```text
> {"op":"hello","protocol":1}
< {"type":"ready","protocol":1}
> {"op":"create","id":1,"type":"div","props":{"class":"flex flex-col items-center gap-4 p-6 size-full"}}
> {"op":"create","id":2,"type":"text","props":{"content":"0","class":"text-xl font-bold"}}
> {"op":"create","id":3,"type":"Button","props":{"label":"Add","variant":"primary"},"events":["click"]}
> {"op":"append","parent":1,"child":2}
> {"op":"append","parent":1,"child":3}
> {"op":"window","title":"Counter","width":320,"height":200,"root":1}
> {"op":"commit"}
< {"type":"event","id":3,"event":"click"}
> {"op":"setProps","id":2,"props":{"content":"1"}}
> {"op":"commit"}
```

1. **Handshake.** The client sends `hello` with its protocol version, and the host answers
   `ready`. Before that, the host ignores everything else.
2. **Build the tree.** The client picks the node ids, creates nodes, and places them with
   `append` and `insertBefore`. `window` names the root.
3. **Commit.** Operations pile up and are applied together on `commit`. The host only redraws
   after a commit, so a client can batch a whole screen into one frame.
4. **Events.** A node only reports the events it asked for (`"events":["click"]`). The client
   reacts and commits again.
5. **Exit.** Closing stdin ends the host. Closing the last window does too, unless there is a
   tray icon.

## The tree

- **Ids belong to the client.** They are positive integers, unique for the session and never
  reused. The client never waits for the host to hand out an id.
- **`setProps` merges.** Send only what changes. `null` removes a prop.
- **`remove` takes the subtree with it.** `append` on a node that already has a parent moves
  it, as in the DOM.
- **Errors don't stop the batch.** A failing operation is dropped and answered with an `error`
  carrying a `code`. The rest of the commit still applies.

## Styling

Nodes take a `class` string with Tailwind-like utilities: `"flex gap-2 p-4 bg-muted rounded-lg"`.
The colors come from the theme, so light mode, dark mode and custom themes need no extra work.
[Protocol §8](/protocol#_8-classes) lists the supported classes.

## Text lives in the host

While the user types, the host handles the cursor, the selection and IME on its own and reports
`change`. The client does not echo the text back. It only sends `value` when it wants to impose
one, for example to clear a field after `submit`.

## What the host covers

Node types, props and events are in [Protocol §5](/protocol#_5-node-types) and the sections
after it. That includes windows, menus, the tray, keyboard shortcuts, system dialogs,
notifications, accessibility, lists, tables, charts, canvas, docks and chat. For what a component
looks like and how it behaves, see the [gpui-kit documentation](https://gpui-kit.com/).

## Platforms

Prebuilt hosts exist for Windows x64, macOS arm64 and Linux x64 (glibc 2.35 or newer). CI builds
and tests the host on all three. It has been drawn and clicked in a real window on Windows and
Linux (Wayland and X11), but not yet on macOS. The protocol is version 1.

<style>
.arch {
  display: flex;
  align-items: stretch;
  gap: 12px;
  margin: 28px 0;
  flex-wrap: wrap;
}
.arch-box {
  flex: 1 1 180px;
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 18px 20px;
  border-radius: 14px;
  border: 1px solid var(--vp-c-divider);
  background: var(--vp-c-bg-soft);
}
.arch-box span {
  color: var(--vp-c-text-2);
  font-size: 14px;
}
.arch-wire {
  flex: 1 1 220px;
  display: flex;
  flex-direction: column;
  justify-content: center;
  align-items: center;
  gap: 6px;
  font-size: 13px;
  color: var(--vp-c-brand-1);
}
</style>
