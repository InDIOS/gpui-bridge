// Headless checks that React state changes become the right protocol operations.

import { test } from "node:test";
import assert from "node:assert/strict";
import { useState } from "react";
import type { App, ClientMessage, DiffSelection, HostError } from "gpui-bridge";
import { Accordion, Alert, Badge, Button, Calendar, Carousel, Chart, Checkbox, Collapsible, DescriptionList, Dialog, Diff, Div, Field, Form, GroupBox, Icon, Image, Input, Kbd, MenuBar, NumberInput, Pagination, Progress, RadioGroup, Rating, render, Resizable, Select, Sidebar, Skeleton, Slider, Spinner, StatusBar, Stepper, Switch, Table, Tab, Tabs, Tag, Text, Textarea, Toolbar, Tray, Tree, VirtualList, Window } from "../src/index.js";

const tick = () => new Promise((resolve) => setTimeout(resolve, 50));

/** Renders headless and records every message sent to the host. */
async function renderRecorded(element: React.ReactNode) {
  const { app } = await render(element, { headless: true });
  const sent: ClientMessage[] = [];
  const host = (app as unknown as { host: { send(m: ClientMessage): void } }).host;
  const send = host.send.bind(host);
  host.send = (message) => {
    sent.push(message);
    send(message);
  };
  const errors: HostError[] = [];
  app.on("error", (error) => errors.push(error));
  await tick();
  return { app, sent, errors };
}

function created(sent: ClientMessage[], type: string) {
  return sent.filter((m): m is Extract<ClientMessage, { op: "create" }> => m.op === "create" && m.type === type);
}

function emit(app: App, message: { id: number; event: string; value?: string | number | null | { column: string; direction: "asc" | "desc" | "none" }; checked?: boolean }) {
  app.handleMessage({ type: "event", ...message });
}

test("Image takes a path or base64 bytes", async () => {
  const { app, sent, errors } = await renderRecorded(
    <Div>
      <Image src="a.png" fit="cover" />
      <Image data="aGVsbG8=" />
    </Div>,
  );
  const [file, bytes] = created(sent, "Image");
  assert.deepEqual(file.props, { src: "a.png", fit: "cover" });
  assert.deepEqual(bytes.props, { data: "aGVsbG8=" });
  await app.close();
  assert.deepEqual(errors, []);
});

test("MenuBar and contextMenu report the chosen item to onMenu", async () => {
  const chosen: string[] = [];
  const { app, sent, errors } = await renderRecorded(
    <Div contextMenu={[{ id: "copy", label: "Copy" }]} onMenu={(item) => chosen.push(`div:${item}`)}>
      <MenuBar menus={[{ label: "File", items: [{ id: "open", label: "Open" }]}]} onMenu={(item) => chosen.push(`bar:${item}`)} />
    </Div>,
  );
  const [bar] = created(sent, "MenuBar");
  const div = created(sent, "div").find((m) => m.props?.contextMenu);
  assert.ok(div && sent.some((m) => m.op === "setEvents" && m.id === div.id && m.events.includes("menu")));
  emit(app, { id: bar.id, event: "menu", value: "open" });
  emit(app, { id: div.id, event: "menu", value: "copy" });
  await app.close();
  assert.deepEqual(chosen, ["bar:open", "div:copy"]);
  assert.deepEqual(errors, []);
});

test("Tray shows while mounted, updates, reports events and is removed on unmount", async () => {
  let setShown!: (shown: boolean) => void;
  let setTip!: (tip: string) => void;
  const events: string[] = [];
  function Root() {
    const [shown, setS] = useState(true);
    const [tip, setT] = useState("Idle");
    [setShown, setTip] = [setS, setT];
    return (
      <Div>
        <Text>main</Text>
        {shown && (
          <Tray tooltip={tip} menu={[{ id: "quit", label: "Quit" }]} onClick={() => events.push("click")} onMenu={(item) => events.push(item)} />
        )}
      </Div>
    );
  }
  const { app, sent, errors } = await renderRecorded(<Root />);
  const trayOps = () => sent.filter((m) => m.op === "tray" || m.op === "removeTray");
  assert.deepEqual(trayOps(), [{ op: "tray", tooltip: "Idle", menu: [{ id: "quit", label: "Quit" }] }]);

  sent.length = 0;
  setTip("Busy");
  await tick();
  assert.deepEqual(trayOps(), [{ op: "tray", tooltip: "Busy" }]);

  app.handleMessage({ type: "tray", event: "click" });
  app.handleMessage({ type: "tray", event: "menu", value: "quit" });
  assert.deepEqual(events, ["click", "quit"]);

  sent.length = 0;
  setShown(false);
  await tick();
  assert.deepEqual(trayOps(), [{ op: "removeTray" }]);
  assert.equal(sent.at(-1)?.op, "commit");
  app.handleMessage({ type: "tray", event: "click" });
  assert.deepEqual(events, ["click", "quit"]);

  await app.close();
  assert.deepEqual(errors, []);
});

test("form controls: value or checked makes them controlled, onChange gets plain values", async () => {
  const got: unknown[] = [];
  const options = [{ value: "a", label: "A" }, { value: "b", label: "B" }];
  function Form() {
    const [size, setSize] = useState("a");
    return (
      <Div>
        <Switch label="On" onChange={(checked) => got.push(["switch", checked])} />
        <RadioGroup options={options} value={size} onChange={(value) => (got.push(["radio", value]), setSize(value))} />
        <Select options={options} placeholder="Pick" onChange={(value) => got.push(["select", value])} />
        <Slider value={5} max={10} onChange={(value) => got.push(["slider", value])} />
        <Textarea placeholder="Notes" onChange={(value) => got.push(["notes", value])} />
      </Div>
    );
  }
  const { app, sent, errors } = await renderRecorded(<Form />);
  const one = (type: string) => created(sent, type)[0];
  assert.equal(one("Switch").props?.controlled, false);
  assert.equal(one("RadioGroup").props?.controlled, true);
  assert.equal(one("Select").props?.controlled, false);
  assert.equal(one("Slider").props?.controlled, true);
  assert.equal(one("Textarea").props?.controlled, undefined);

  emit(app, { id: one("Switch").id, event: "change", checked: true });
  emit(app, { id: one("RadioGroup").id, event: "change", value: "b" });
  emit(app, { id: one("Select").id, event: "change", value: "a" });
  emit(app, { id: one("Slider").id, event: "change", value: 7 });
  emit(app, { id: one("Textarea").id, event: "change", value: "x" });
  await tick();
  assert.deepEqual(got, [["switch", true], ["radio", "b"], ["select", "a"], ["slider", 7], ["notes", "x"]]);
  // The controlled RadioGroup confirms the choice.
  assert.ok(sent.some((m) => m.op === "setProps" && m.id === one("RadioGroup").id && m.props.value === "b"));
  await app.close();
  assert.deepEqual(errors, []);
});

test("onKeyDown and onDoubleClick subscribe and get plain values; a ref focuses", async () => {
  const got: unknown[] = [];
  let input: any;
  const { app, sent, errors } = await renderRecorded(
    <Div onKeyDown={(key) => got.push(["key", key])} onDoubleClick={() => got.push(["double"])}>
      <Input ref={(node: any) => (input = node)} onKeyDown={(key) => got.push(["input", key])} />
    </Div>,
  );
  const div = created(sent, "div").find((m) => sent.some((e) => e.op === "setEvents" && e.id === m.id && e.events.includes("keyDown")));
  const [field] = created(sent, "Input");
  assert.ok(div);
  emit(app, { id: div.id, event: "keyDown", value: "escape" });
  emit(app, { id: div.id, event: "doubleClick" });
  emit(app, { id: field.id, event: "keyDown", value: "down" });
  assert.deepEqual(got, [["key", "escape"], ["double"], ["input", "down"]]);

  sent.length = 0;
  input.focus();
  app.commit();
  assert.deepEqual(sent, [{ op: "focus", id: field.id }, { op: "commit" }]);
  await app.close();
  assert.deepEqual(errors, []);
});

test("onDrop gets the dropped paths", async () => {
  const got: string[][] = [];
  const { app, sent, errors } = await renderRecorded(<Div onDrop={(paths) => got.push(paths)} />);
  const div = created(sent, "div").find((m) => sent.some((e) => e.op === "setEvents" && e.id === m.id && e.events.includes("drop")));
  assert.ok(div);
  app.handleMessage({ type: "event", id: div.id, event: "drop", paths: ["C:\\a.txt"] });
  assert.deepEqual(got, [["C:\\a.txt"]]);
  await app.close();
  assert.deepEqual(errors, []);
});

test("Dialog renders its children as a node with onOk/onCancel; tooltip passes through", async () => {
  const got: string[] = [];
  function Rename() {
    const [open, setOpen] = useState(true);
    return (
      <Div tooltip="Area">
        <Dialog open={open} title="Rename" okText="Save" onOk={() => (got.push("ok"), setOpen(false))} onCancel={() => got.push("cancel")}>
          <Input placeholder="Name" />
        </Dialog>
      </Div>
    );
  }
  const { app, sent, errors } = await renderRecorded(<Rename />);
  const [dialog] = created(sent, "Dialog");
  const [input] = created(sent, "Input");
  assert.deepEqual(dialog.props, { open: true, title: "Rename", okText: "Save" });
  assert.ok(sent.some((m) => m.op === "append" && m.parent === dialog.id && m.child === input.id));
  assert.equal(created(sent, "div").find((m) => m.props?.tooltip)?.props?.tooltip, "Area");

  sent.length = 0;
  emit(app, { id: dialog.id, event: "ok" });
  await tick();
  assert.deepEqual(got, ["ok"]);
  assert.ok(sent.some((m) => m.op === "setProps" && m.id === dialog.id && m.props.open === false));
  await app.close();
  assert.deepEqual(errors, []);
});

test("VirtualList renders rows as children; Table and Tabs get plain values", async () => {
  const got: unknown[] = [];
  const columns = [{ key: "name", name: "Name", sortable: true }];
  function Data() {
    const [rows, setRows] = useState([{ name: "b" }, { name: "a" }]);
    const [tab, setTab] = useState("list");
    return (
      <Div>
        <Tabs tabs={[{ value: "list", label: "List" }, { value: "table", label: "Table" }]} value={tab} onChange={(value) => (got.push(["tab", value]), setTab(value))} />
        <VirtualList rowHeight={24} class="h-64">
          {rows.map((row) => <Text key={row.name}>{row.name}</Text>)}
        </VirtualList>
        <Table
          columns={columns}
          rows={rows}
          onChange={(row) => got.push(["row", row])}
          onDoubleClick={(row) => got.push(["open", row])}
          onSort={(sort) => (got.push(["sort", sort]), setRows([...rows].sort((x, y) => x.name.localeCompare(y.name))))}
        />
      </Div>
    );
  }
  const { app, sent, errors } = await renderRecorded(<Data />);
  const [list] = created(sent, "VirtualList");
  const [table] = created(sent, "Table");
  const [tabs] = created(sent, "Tabs");
  assert.equal(list.props?.rowHeight, 24);
  assert.equal(sent.filter((m) => m.op === "append" && m.parent === list.id).length, 2);
  assert.equal(table.props?.controlled, false);
  assert.equal(tabs.props?.controlled, true);

  emit(app, { id: table.id, event: "change", value: 1 });
  emit(app, { id: table.id, event: "change", value: null });
  emit(app, { id: table.id, event: "doubleClick", value: 0 });
  emit(app, { id: table.id, event: "sort", value: { column: "name", direction: "asc" } });
  emit(app, { id: tabs.id, event: "change", value: "table" });
  await tick();
  assert.deepEqual(got, [["row", 1], ["row", null], ["open", 0], ["sort", { column: "name", direction: "asc" }], ["tab", "table"]]);
  // Sorted rows and the confirmed tab go back to the host.
  assert.ok(sent.some((m) => m.op === "setProps" && m.id === table.id && JSON.stringify(m.props.rows) === JSON.stringify([{ name: "a" }, { name: "b" }])));
  assert.ok(sent.some((m) => m.op === "setProps" && m.id === tabs.id && m.props.value === "table"));
  await app.close();
  assert.deepEqual(errors, []);
});

test("a Tabs of Tab nodes puts their children in the chips", async () => {
  const got: unknown[] = [];
  function Chips() {
    const [tab, setTab] = useState("files");
    return (
      <Tabs value={tab} variant="underline" onChange={(value) => (got.push(["tab", value]), setTab(value))}>
        <Tab value="files" a11y={{ label: "Files" }}>
          <Icon name="folder" />
          <Text>Files</Text>
          <Badge count={3} />
        </Tab>
        <Tab value="history" label="History" disabled />
      </Tabs>
    );
  }
  const { app, sent, errors } = await renderRecorded(<Chips />);
  const [tabs] = created(sent, "Tabs");
  const chips = created(sent, "Tab");
  assert.deepEqual(chips.map((m) => m.props), [
    { value: "files", a11y: { label: "Files" } },
    { value: "history", label: "History", disabled: true },
  ]);
  assert.ok(sent.some((m) => m.op === "append" && m.parent === tabs.id && m.child === chips[0].id));
  // The chip is built from nodes: its `Icon`, `Text` and `Badge` are inside the `Tab`.
  assert.equal(sent.filter((m) => m.op === "append" && m.parent === chips[0].id).length, 3);

  emit(app, { id: tabs.id, event: "change", value: "history" });
  await tick();
  assert.deepEqual(got, [["tab", "history"]]);
  // The confirmed value goes back to the `Tabs`, which has no `tabs` list at all.
  assert.ok(sent.some((m) => m.op === "setProps" && m.id === tabs.id && m.props.value === "history"));
  await app.close();
  assert.deepEqual(errors, []);
});

test("Tree, Diff and Carousel render with their props and report plain values", async () => {
  const got: unknown[] = [];
  const items = [
    { id: "src", label: "src", expanded: true, children: [{ id: "src/lib.rs", label: "lib.rs" }] },
    { id: "README.md", label: "README.md" },
  ];
  const patch = "diff --git a/notes.txt b/notes.txt\n--- a/notes.txt\n+++ b/notes.txt\n@@ -1,3 +1,4 @@\n one\n-two\n+2\n three\n+four\n";
  function Advanced() {
    const [selected, setSelected] = useState("src");
    const [slide, setSlide] = useState(0);
    return (
      <Div>
        <Tree
          items={items}
          value={selected}
          onChange={(id) => (got.push(["tree", id]), setSelected(id))}
          onExpand={(id) => got.push(["expand", id])}
          onCollapse={(id) => got.push(["collapse", id])}
        />
        <Diff patch={patch} mode="split" onChange={(selection) => got.push(["diff", selection])} />
        <Carousel value={slide} pagination onChange={(index) => (got.push(["slide", index]), setSlide(index))}>
          <Text>One</Text>
          <Text>Two</Text>
        </Carousel>
      </Div>
    );
  }
  const { app, sent, errors } = await renderRecorded(<Advanced />);
  const [tree] = created(sent, "Tree");
  const [diff] = created(sent, "Diff");
  const [carousel] = created(sent, "Carousel");
  assert.deepEqual(tree.props?.items, items);
  assert.equal(tree.props?.controlled, true);
  assert.equal(diff.props?.mode, "split");
  assert.equal(carousel.props?.pagination, true);
  assert.equal(carousel.props?.controlled, true);
  assert.equal(sent.filter((m) => m.op === "append" && m.parent === carousel.id).length, 2);

  const selection: DiffSelection = { path: "notes.txt", side: "modified", start: 1, end: 2, endSide: "modified" };
  emit(app, { id: tree.id, event: "change", value: "README.md" });
  emit(app, { id: tree.id, event: "expand", value: "src" });
  emit(app, { id: tree.id, event: "collapse", value: "src" });
  app.handleMessage({ type: "event", id: diff.id, event: "change", value: selection });
  app.handleMessage({ type: "event", id: diff.id, event: "change", value: null });
  emit(app, { id: carousel.id, event: "change", value: 1 });
  await tick();
  assert.deepEqual(got, [
    ["tree", "README.md"],
    ["expand", "src"],
    ["collapse", "src"],
    ["diff", selection],
    ["diff", null],
    ["slide", 1],
  ]);
  assert.ok(sent.some((m) => m.op === "setProps" && m.id === tree.id && m.props.value === "README.md"));
  assert.ok(sent.some((m) => m.op === "setProps" && m.id === carousel.id && m.props.value === 1));
  await app.close();
  assert.deepEqual(errors, []);
});

test("Sidebar and Chart render with their props and report plain values", async () => {
  const got: unknown[] = [];
  const items = [
    {
      label: "General",
      items: [
        { id: "home", label: "Home", icon: "layout-dashboard" },
        { id: "inbox", label: "Inbox" },
      ],
    },
    { items: [{ id: "settings", label: "Settings", children: [{ id: "theme", label: "Theme" }] }] },
  ];
  const data = [
    { label: "Mon", value: 12 },
    { label: "Tue", value: 18 },
  ];
  function Dashboard() {
    const [active, setActive] = useState("home");
    return (
      <Div>
        <Sidebar items={items} value={active} onChange={(id) => (got.push(["sidebar", id]), setActive(id))} />
        <Chart chart="line" data={data} yAxis />
        <Chart chart="pie" data={[{ label: "Rust", value: 60 }]} innerRadius={20} />
      </Div>
    );
  }
  const { app, sent, errors } = await renderRecorded(<Dashboard />);
  const [sidebar] = created(sent, "Sidebar");
  const charts = created(sent, "Chart");
  assert.deepEqual(sidebar.props?.items, items);
  assert.equal(sidebar.props?.controlled, true);
  assert.equal(charts.length, 2);
  assert.equal(charts[0].props?.chart, "line");
  assert.equal(charts[0].props?.yAxis, true);
  assert.deepEqual(charts[0].props?.data, data);
  assert.equal(charts[1].props?.innerRadius, 20);

  emit(app, { id: sidebar.id, event: "change", value: "theme" });
  await tick();
  assert.deepEqual(got, [["sidebar", "theme"]]);
  assert.ok(sent.some((m) => m.op === "setProps" && m.id === sidebar.id && m.props.value === "theme"));
  await app.close();
  assert.deepEqual(errors, []);
});

test("state updates become setProps and commit", async () => {
  function Counter() {
    const [count, setCount] = useState(0);
    return (
      <Div class="flex">
        <Text>Clicks: {count}</Text>
        <Button onClick={() => setCount(count + 1)}>Add</Button>
      </Div>
    );
  }
  const { app, sent, errors } = await renderRecorded(<Counter />);
  const [text] = created(sent, "text");
  const [button] = created(sent, "Button");
  assert.equal(text.props?.content, "Clicks: 0");
  assert.equal(button.props?.label, "Add");
  assert.ok(sent.some((m) => m.op === "setEvents" && m.id === button.id && m.events.includes("click")));
  assert.equal(sent.at(-1)?.op, "commit");

  sent.length = 0;
  emit(app, { id: button.id, event: "click" });
  await tick();
  assert.deepEqual(sent, [{ op: "setProps", id: text.id, props: { content: "Clicks: 1" } }, { op: "commit" }]);

  await app.close();
  assert.deepEqual(errors, []);
});

test("value and checked make inputs controlled by React state", async () => {
  function Form() {
    const [draft, setDraft] = useState("");
    const [done, setDone] = useState(false);
    return (
      <Div>
        <Input value={draft} onChange={(value) => setDraft(value.toUpperCase())} />
        <Checkbox checked={done} onChange={setDone} />
      </Div>
    );
  }
  const { app, sent, errors } = await renderRecorded(<Form />);
  const [input] = created(sent, "Input");
  const [checkbox] = created(sent, "Checkbox");
  assert.equal(input.props?.controlled, true);
  assert.equal(checkbox.props?.controlled, true);

  sent.length = 0;
  emit(app, { id: input.id, event: "change", value: "abc" });
  emit(app, { id: checkbox.id, event: "change", checked: true });
  await tick();
  const updates = sent.filter((m) => m.op === "setProps");
  assert.deepEqual(updates, [
    { op: "setProps", id: input.id, props: { value: "ABC" } },
    { op: "setProps", id: checkbox.id, props: { checked: true } },
  ]);

  await app.close();
  assert.deepEqual(errors, []);
});

test("removing elements and handlers maps to remove and setEvents", async () => {
  let setItems!: (items: string[]) => void;
  let setClickable!: (on: boolean) => void;
  function List() {
    const [items, set] = useState(["a", "b", "c"]);
    const [clickable, setOn] = useState(true);
    setItems = set;
    setClickable = setOn;
    return (
      <Div onClick={clickable ? () => {} : undefined}>
        {items.map((item) => (
          <Text key={item}>{item}</Text>
        ))}
      </Div>
    );
  }
  const { app, sent, errors } = await renderRecorded(<List />);
  const texts = created(sent, "text");
  const div = created(sent, "div").at(-1)!;

  sent.length = 0;
  setItems(["a", "c"]);
  setClickable(false);
  await tick();
  assert.ok(sent.some((m) => m.op === "remove" && m.id === texts[1].id));
  assert.ok(sent.some((m) => m.op === "setEvents" && m.id === div.id && m.events.length === 0));

  await app.close();
  assert.deepEqual(errors, []);
});

test("Icon renders as an Icon node and changes its name", async () => {
  function Toggle() {
    const [open, setOpen] = useState(false);
    return (
      <Div onClick={() => setOpen(!open)}>
        <Icon name={open ? "folder-open" : "folder"} class="size-4" />
      </Div>
    );
  }
  const { app, sent, errors } = await renderRecorded(<Toggle />);
  const [icon] = created(sent, "Icon");
  const [div] = created(sent, "div").slice(-1);
  assert.deepEqual(icon.props, { name: "folder", class: "size-4" });

  sent.length = 0;
  emit(app, { id: div.id, event: "click" });
  await tick();
  assert.deepEqual(sent, [{ op: "setProps", id: icon.id, props: { name: "folder-open" } }, { op: "commit" }]);

  await app.close();
  assert.deepEqual(errors, []);
});

test("Window opens while mounted, updates, and closes when unmounted", async () => {
  let setOpen!: (open: boolean) => void;
  let setTitle!: (title: string) => void;
  let setWrapped!: (wrapped: boolean) => void;
  const closes: string[] = [];
  function Root() {
    const [open, setO] = useState(false);
    const [title, setT] = useState("Settings");
    const [wrapped, setW] = useState(true);
    [setOpen, setTitle, setWrapped] = [setO, setT, setW];
    return (
      <Div>
        <Text>main</Text>
        {open && (
          <Window title={title} width={300} onClose={() => closes.push("user")}>
            <Text>inside</Text>
          </Window>
        )}
        {wrapped && (
          <Div>
            <Window title="Nested">
              <Text>nested</Text>
            </Window>
          </Div>
        )}
      </Div>
    );
  }
  const { app, sent, errors } = await renderRecorded(<Root />);
  // Window messages other than the main window (render() sends that one without an id).
  const windowOps = () => sent.filter((m) => (m.op === "window" && m.id !== undefined) || m.op === "closeWindow");
  // The nested window opens with its own root; its root is not appended anywhere.
  const [nested] = windowOps() as Extract<ClientMessage, { op: "window" }>[];
  assert.equal(nested.title, "Nested");
  assert.ok(nested.id! >= 2 && nested.root !== undefined);
  assert.ok(!sent.some((m) => m.op === "append" && m.child === nested.root));

  sent.length = 0;
  setOpen(true);
  await tick();
  const [opened] = windowOps() as Extract<ClientMessage, { op: "window" }>[];
  assert.deepEqual([opened.title, opened.width], ["Settings", 300]);

  sent.length = 0;
  setTitle("Preferences");
  await tick();
  assert.deepEqual(windowOps(), [{ op: "window", id: opened.id, title: "Preferences" }]);

  // The user closes it: onClose runs; unmounting afterwards sends no closeWindow (the host forgot it).
  app.handleMessage({ type: "windowClosed", id: opened.id! });
  assert.deepEqual(closes, ["user"]);
  sent.length = 0;
  setOpen(false);
  await tick();
  assert.deepEqual(windowOps(), []);

  // Removing an ancestor closes the window inside it.
  sent.length = 0;
  setWrapped(false);
  await tick();
  assert.deepEqual(windowOps(), [{ op: "closeWindow", id: nested.id }]);
  assert.ok(sent.some((m) => m.op === "remove" && m.id === nested.root));
  assert.equal(sent.at(-1)?.op, "commit");

  await app.close();
  assert.deepEqual(errors, []);
});

test("the new components render with their props and report their events", async () => {
  const seen: unknown[] = [];
  const { app, sent, errors } = await renderRecorded(
    <Div>
      <Alert message="Saved" variant="success" closable onClose={() => seen.push("close")} />
      <Badge count={3}>
        <Icon name="bell" />
      </Badge>
      <Tag label="beta" variant="info" />
      <Kbd keys="ctrl-s" />
      <Progress value={40} />
      <Spinner class="size-4" />
      <Skeleton class="h-4 w-32" />
      <Rating value={2} onChange={(value) => seen.push(["rating", value])} />
      <Pagination page={1} total={4} onChange={(page) => seen.push(["page", page])} />
      <Collapsible open>
        <Text>Hidden</Text>
      </Collapsible>
      <Accordion items={[{ id: "a", title: "First", open: true }]} onChange={(id) => seen.push(["accordion", id])} />
      <Stepper value="one" items={[{ value: "one", label: "One" }, { value: "two", label: "Two" }]} onChange={(value) => seen.push(["step", value])} />
      <NumberInput value="12" onChange={(value) => seen.push(["number", value])} />
      <Form direction="vertical">
        <Field label="Name" required>
          <Input placeholder="Ana" />
        </Field>
      </Form>
      <GroupBox title="Details">
        <DescriptionList items={[{ label: "Plan", value: "Pro" }]} />
      </GroupBox>
      <Toolbar>
        <Button label="Bold" />
      </Toolbar>
      <Resizable axis="horizontal" sizes={[200]}>
        <Div a11y={{ role: "region", label: "Side" }} />
        <Div a11y={{ role: "region", label: "Content" }} />
      </Resizable>
      <StatusBar left="Ready" right="UTF-8" />
      <Calendar value="2024-08-03" onChange={(day) => seen.push(["day", day])} />
    </Div>,
  );
  const [alert] = created(sent, "Alert");
  const [rating] = created(sent, "Rating");
  const [pages] = created(sent, "Pagination");
  const [accordion] = created(sent, "Accordion");
  const [steps] = created(sent, "Stepper");
  const [number] = created(sent, "NumberInput");
  const [field] = created(sent, "Field");
  const [calendar] = created(sent, "Calendar");
  const [group] = created(sent, "GroupBox");
  // A `value` makes a node controlled in React, as it does in the DOM.
  assert.deepEqual(rating.props, { value: 2, controlled: true });
  assert.deepEqual(steps.props, { value: "one", items: [{ value: "one", label: "One" }, { value: "two", label: "Two" }], controlled: true });
  assert.deepEqual(number.props, { value: "12", controlled: true });
  assert.deepEqual(field.props, { label: "Name", required: true });
  assert.deepEqual(group.props, { title: "Details" });
  // The accessibility prop travels as written.
  const region = created(sent, "div").find((m) => m.props?.a11y);
  assert.deepEqual(region?.props?.a11y, { role: "region", label: "Side" });

  emit(app, { id: alert.id, event: "close" });
  emit(app, { id: rating.id, event: "change", value: 4 });
  emit(app, { id: pages.id, event: "change", value: 2 });
  emit(app, { id: accordion.id, event: "change", value: "a" });
  emit(app, { id: steps.id, event: "change", value: "two" });
  emit(app, { id: number.id, event: "change", value: "13" });
  emit(app, { id: calendar.id, event: "change", value: "2024-09-01" });
  await app.close();
  assert.deepEqual(seen, ["close", ["rating", 4], ["page", 2], ["accordion", "a"], ["step", "two"], ["number", "13"], ["day", "2024-09-01"]]);
  assert.deepEqual(errors, []);
});

test("Window handlers opt into window events and receive their payload", async () => {
  const resized: number[] = [];
  const moved: Array<[number | undefined, number | undefined]> = [];
  const { app, sent, errors } = await renderRecorded(
    <Div>
      <Text>main</Text>
      <Window
        title="Settings"
        onResize={(bounds) => resized.push(bounds.width ?? 0)}
        onMove={(bounds) => moved.push([bounds.x, bounds.y])}
      >
        <Text>inside</Text>
      </Window>
    </Div>,
  );

  const windowOps = sent.filter(
    (m): m is Extract<ClientMessage, { op: "window" }> => m.op === "window" && m.id !== undefined,
  );
  const subscribed = windowOps.at(-1)!;
  assert.deepEqual([...subscribed.events!].sort(), ["move", "resize"]);
  const id = windowOps[0].id!;

  app.handleMessage({ type: "windowEvent", id, event: "resize", x: 40, y: 60, width: 512, height: 384 });
  app.handleMessage({ type: "windowEvent", id, event: "move", x: 20, y: 30, width: 512, height: 384 });
  assert.deepEqual(resized, [512]);
  assert.deepEqual(moved, [[20, 30]]);
  assert.deepEqual(errors, []);
  await app.close();
});

