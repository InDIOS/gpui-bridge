// Runs against the real host binary in headless mode (`npm test` builds first).

import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { createApp, defaultHostPath, HostExitedError, type HostError, type HostExit } from "../dist/index.js";

const headless = { headless: true };

test("handshake completes", async () => {
  const app = await createApp(headless);
  const exit = await app.close();
  assert.equal(exit.code, 0);
});

test("a valid tree commits without errors", async () => {
  const app = await createApp(headless);
  const errors: HostError[] = [];
  app.on("error", (error) => errors.push(error));

  const root = app.div({ class: "flex flex-col gap-2 p-4" });
  const label = app.text({ content: "Clicks: 0" });
  const button = app.button({ label: "Add", variant: "primary" }).on("click", () => {});
  const input = app.input({ placeholder: "Type" }).on("change", () => {}).on("submit", () => {});
  const checkbox = app.checkbox({ label: "Done", controlled: true }).on("change", () => {});
  root.append(label, button, input, checkbox);
  app.window({ title: "Test", width: 400, height: 300, root, theme: "dark" });
  app.commit();

  label.set({ content: "Clicks: 1" });
  button.set({ variant: null, disabled: true }).off("click");
  checkbox.remove();
  app.commit();

  // The host handles lines in order, so every reply precedes its exit.
  await app.close();
  assert.deepEqual(errors, []);
});

test("an image can carry base64 bytes instead of a file path", async () => {
  const app = await createApp(headless);
  const errors: HostError[] = [];
  app.on("error", (error) => errors.push(error));

  const image = app.image({ data: Buffer.from("hello").toString("base64"), fit: "cover" });
  app.window({ root: app.div().append(image) });
  app.commit();

  // Bytes sent again reload the node; a bad payload is rejected and the previous bytes stay.
  image.set({ data: Buffer.from("world").toString("base64") });
  app.commit();
  image.set({ data: "not base64!" });
  app.commit();

  await app.close();
  assert.deepEqual(
    errors.map((error) => [error.code, error.message]),
    [["invalid_props", "Prop 'data' of Image must be standard base64"]],
  );
});

test("host errors arrive typed and the host keeps running", async () => {
  const app = await createApp(headless);
  const errors: HostError[] = [];
  app.on("error", (error) => errors.push(error));

  const root = app.div();
  const gone = app.text({ content: "gone" });
  gone.remove();
  root.append(gone); // the host no longer knows this node
  app.commit();

  const button = app.button({ label: "x" });
  button.set({ disabled: "yes" as unknown as boolean });
  app.commit();

  await app.close();
  assert.deepEqual(errors, [
    { code: "unknown_node", message: `Node ${gone.id} does not exist`, op: "append" },
    { code: "invalid_props", message: 'Prop \'disabled\' of Button must be a boolean, got "yes"', op: "setProps" },
  ]);
});

test("events are dispatched with typed payloads", async () => {
  const app = await createApp(headless);
  const seen: unknown[] = [];
  const button = app.button().on("click", (payload) => seen.push(["click", payload]));
  const input = app.input().on("change", ({ value }) => seen.push(["change", value]));
  const checkbox = app.checkbox().on("change", ({ checked }) => seen.push(["checked", checked]));

  // Simulate host events (real clicks need a window; see the end-to-end check).
  app.handleMessage({ type: "event", id: button.id, event: "click" });
  app.handleMessage({ type: "event", id: input.id, event: "change", value: "hi" });
  app.handleMessage({ type: "event", id: checkbox.id, event: "change", checked: true });
  input.off("change");
  app.handleMessage({ type: "event", id: input.id, event: "change", value: "ignored" });

  await app.close();
  assert.deepEqual(seen, [["click", {}], ["change", "hi"], ["checked", true]]);
});

test("menus are accepted by the host and report the chosen item", async () => {
  const app = await createApp(headless);
  const errors: HostError[] = [];
  app.on("error", (error) => errors.push(error));
  const seen: unknown[] = [];
  const bar = app
    .menuBar({ menus: [{ label: "File", items: [{ id: "open", label: "Open", icon: "folder-open" }, { separator: true }, { label: "Recent", items: [{ id: "a", label: "a.txt" }] }] }] })
    .on("menu", ({ item }) => seen.push(["bar", item]));
  const area = app.div({ contextMenu: [{ id: "copy", label: "Copy", checked: true }] }).on("menu", ({ item }) => seen.push(["area", item]));
  app.window({ root: app.div().append(bar, area) });
  app.commit();

  app.handleMessage({ type: "event", id: bar.id, event: "menu", value: "a" });
  app.handleMessage({ type: "event", id: area.id, event: "menu", value: "copy" });
  await app.close();
  assert.deepEqual(errors, []);
  assert.deepEqual(seen, [["bar", "a"], ["area", "copy"]]);
});

test("the tray reports events and removing it with no window ends the host", async () => {
  const app = await createApp(headless);
  const errors: HostError[] = [];
  app.on("error", (error) => errors.push(error));
  const seen: unknown[] = [];
  app.on("tray", (event) => seen.push(event));
  app.tray({ tooltip: "Test", menu: [{ id: "quit", label: "Quit" }] }).commit();
  app.tray({ menu: [{ label: "No id" } as never] }).commit();

  app.handleMessage({ type: "tray", event: "click" });
  app.handleMessage({ type: "tray", event: "menu", value: "quit" });
  app.removeTray().commit();
  const exit = await app.exited();
  assert.equal(exit.code, 0);
  assert.deepEqual(errors.map((e) => [e.code, e.op]), [["invalid_json", "tray"]]);
  assert.deepEqual(seen, [{ event: "click" }, { event: "menu", item: "quit" }]);
});

test("form controls are accepted by the host and report typed values", async () => {
  const app = await createApp(headless);
  const errors: HostError[] = [];
  app.on("error", (error) => errors.push(error));
  const seen: unknown[] = [];
  const options = [{ value: "a", label: "A" }, { value: "b", label: "B", disabled: true }];
  const toggle = app.switch({ label: "On", checked: true }).on("change", ({ checked }) => seen.push(["switch", checked]));
  const radios = app.radioGroup({ options, value: "a", direction: "horizontal" }).on("change", ({ value }) => seen.push(["radio", value]));
  const select = app.select({ options, placeholder: "Pick", searchable: true }).on("change", ({ value }) => seen.push(["select", value]));
  const slider = app.slider({ min: 0, max: 1, step: 0.1, value: 0.5 }).on("change", ({ value }) => seen.push(["slider", value]));
  const notes = app.textarea({ placeholder: "Notes" }).on("change", ({ value }) => seen.push(["notes", value]));
  app.window({ root: app.div().append(toggle, radios, select, slider, notes) });
  app.commit();

  app.handleMessage({ type: "event", id: toggle.id, event: "change", checked: false });
  app.handleMessage({ type: "event", id: radios.id, event: "change", value: "a" });
  app.handleMessage({ type: "event", id: select.id, event: "change", value: "a" });
  app.handleMessage({ type: "event", id: slider.id, event: "change", value: 0.3 });
  app.handleMessage({ type: "event", id: notes.id, event: "change", value: "1\n2" });
  await app.close();
  assert.deepEqual(errors, []);
  assert.deepEqual(seen, [["switch", false], ["radio", "a"], ["select", "a"], ["slider", 0.3], ["notes", "1\n2"]]);
});

test("the form controls with state are accepted and report typed values", async () => {
  const app = await createApp(headless);
  const errors: HostError[] = [];
  app.on("error", (error) => errors.push(error));
  const seen: unknown[] = [];
  const options = [{ value: "rs", label: "Rust" }, { value: "go", label: "Go" }, { value: "py", label: "Python", disabled: true }];
  const day = app.datePicker({ value: "2024-08-03", placeholder: "Pick a day" }).on("change", ({ value }) => seen.push(["day", value]));
  const range = app.datePicker({ range: true }).on("change", ({ value }) => seen.push(["range", value]));
  const time = app.timeField({ value: "09:30" }).on("change", ({ value }) => seen.push(["time", value]));
  const color = app
    .colorPicker({ value: "#3366ff", palette: ["#ff0000", "#00ff0080"] })
    .on("change", ({ value }) => seen.push(["color", value]));
  const langs = app
    .combobox({ options, value: ["rs"], placeholder: "Languages", searchable: true })
    .on("change", ({ value }) => seen.push(["langs", value]));
  app.window({ root: app.div().append(day, range, time, color, langs) });
  app.commit();

  app.handleMessage({ type: "event", id: day.id, event: "change", value: "2024-08-10" });
  app.handleMessage({ type: "event", id: day.id, event: "change", value: null });
  app.handleMessage({ type: "event", id: range.id, event: "change", value: "2024-08-01/2024-08-15" });
  app.handleMessage({ type: "event", id: time.id, event: "change", value: "18:05" });
  app.handleMessage({ type: "event", id: color.id, event: "change", value: "#FF0000" });
  app.handleMessage({ type: "event", id: color.id, event: "change", value: null });
  app.handleMessage({ type: "event", id: langs.id, event: "change", value: ["rs", "go"] });
  app.handleMessage({ type: "event", id: langs.id, event: "change", value: ["go"] });
  await app.close();
  assert.deepEqual(errors, []);
  assert.deepEqual(seen, [
    ["day", "2024-08-10"],
    ["day", null],
    ["range", "2024-08-01/2024-08-15"],
    ["time", "18:05"],
    ["color", "#FF0000"],
    ["color", null],
    ["langs", ["rs", "go"]],
    ["langs", ["go"]],
  ]);
});

test("the basic, form and layout components are accepted and report their events", async () => {
  const app = await createApp(headless);
  const errors: HostError[] = [];
  app.on("error", (error) => errors.push(error));
  const seen: unknown[] = [];

  const alert = app.alert({ message: "Saved", variant: "success", closable: true }).on("close", () => seen.push(["alert"]));
  const rating = app.rating({ value: 2 }).on("change", ({ value }) => seen.push(["rating", value]));
  const pagination = app.pagination({ page: 1, total: 4 }).on("change", ({ value }) => seen.push(["page", value]));
  const accordion = app
    .accordion({ items: [{ id: "a", title: "First", content: "Body", open: true }, { id: "b", title: "Second" }] })
    .on("change", ({ value }) => seen.push(["accordion", value]));
  const stepper = app
    .stepper({ value: "one", items: [{ value: "one", label: "One" }, { value: "two", label: "Two" }] })
    .on("change", ({ value }) => seen.push(["step", value]));
  const number = app.numberInput({ value: "12", placeholder: "Qty" }).on("change", ({ value }) => seen.push(["number", value]));
  const otp = app.otpInput({ length: 6, masked: true }).on("change", ({ value }) => seen.push(["otp", value]));
  const calendar = app.calendar({ value: "2024-08-03" }).on("change", ({ value }) => seen.push(["day", value]));

  const field = app.field({ label: "Name", required: true }).append(app.input({ placeholder: "Ana" }));
  const form = app.form({ direction: "vertical" }).append(field);
  const form2 = app.form().append(app.input({ placeholder: "Second" }));
  const details = app.groupBox({ title: "Details" }).append(app.descriptionList({ items: [{ label: "Plan", value: "Pro" }] }));
  const bar = app.toolbar().append(app.button({ label: "Bold", variant: "ghost" }));
  const panels = app.resizable({ axis: "horizontal", sizes: [200] }).append(
    app.div({ a11y: { role: "region", label: "Side" } }),
    app.div({ a11y: { role: "region", label: "Content" } }),
  );
  const footer = app.statusBar({ left: "Ready", right: "UTF-8" });

  app.window({
    root: app.div({ class: "flex flex-col gap-2 p-4" }).append(
      alert,
      app.badge({ count: 3 }),
      app.tag({ label: "beta", variant: "info" }),
      app.kbd({ keys: "ctrl-s" }),
      app.progress({ value: 40 }),
      app.spinner({ class: "size-4" }),
      app.skeleton({ class: "h-4 w-32" }),
      rating,
      pagination,
      app.collapsible({ open: true }).append(app.text({ content: "Hidden" })),
      accordion,
      app.avatar({ name: "Ana" }),
      stepper,
      number,
      otp,
      calendar,
      form,
      form2,
      details,
      bar,
      panels,
      footer,
    ),
  });
  app.commit();

  app.handleMessage({ type: "event", id: alert.id, event: "close" });
  app.handleMessage({ type: "event", id: rating.id, event: "change", value: 4 });
  app.handleMessage({ type: "event", id: pagination.id, event: "change", value: 2 });
  app.handleMessage({ type: "event", id: accordion.id, event: "change", value: "b" });
  app.handleMessage({ type: "event", id: stepper.id, event: "change", value: "two" });
  app.handleMessage({ type: "event", id: number.id, event: "change", value: "13" });
  app.handleMessage({ type: "event", id: otp.id, event: "change", value: "123456" });
  app.handleMessage({ type: "event", id: calendar.id, event: "change", value: "2024-09-01" });
  await app.close();
  assert.deepEqual(errors, []);
  assert.deepEqual(seen, [
    ["alert"],
    ["rating", 4],
    ["page", 2],
    ["accordion", "b"],
    ["step", "two"],
    ["number", "13"],
    ["otp", "123456"],
    ["day", "2024-09-01"],
  ]);
});

test("the simple components are accepted and report their events", async () => {
  const app = await createApp(headless);
  const errors: HostError[] = [];
  const seen: unknown[] = [];
  app.on("error", (error) => errors.push(error));

  const toggle = app
    .toggle({ label: "Bold", icon: "asterisk", variant: "outline", tooltip: "Bold" })
    .on("change", ({ checked }) => seen.push(["toggle", checked]));
  const dropdown = app
    .dropdownButton({ label: "Save", variant: "primary", items: [{ id: "save-as", label: "Save as" }], menuLabel: "Save options" })
    .on("click", () => seen.push(["save"]))
    .on("menu", ({ item }) => seen.push(["menu", item]));
  const group = app
    .inputGroup({ value: "example.com", placeholder: "host", prefix: "https://", suffixIcon: "check" })
    .on("change", ({ value }) => seen.push(["group", value]))
    .on("submit", ({ value }) => seen.push(["submit", value]));
  const clipboard = app
    .clipboard({ value: "https://example.com", a11y: { label: "Copy the address" } })
    .on("copied", ({ value }) => seen.push(["copied", value]));

  app.window({
    root: app.div({ class: "flex flex-col gap-2 p-4" }).append(
      toggle,
      dropdown,
      app.label({ content: "Total", secondary: "(12)", highlight: "12" }),
      app
        .empty({ icon: "folder-open", title: "No files", description: "Drop a file here" })
        .append(app.button({ label: "Choose a file" })),
      app.shimmer({ content: "Loading...", duration: 3, spread: 0.4, reverse: true, once: true }),
      group,
      clipboard,
    ),
  });
  app.commit();

  app.handleMessage({ type: "event", id: toggle.id, event: "change", checked: true });
  app.handleMessage({ type: "event", id: dropdown.id, event: "click" });
  app.handleMessage({ type: "event", id: dropdown.id, event: "menu", value: "save-as" });
  app.handleMessage({ type: "event", id: group.id, event: "change", value: "example.org" });
  app.handleMessage({ type: "event", id: group.id, event: "submit", value: "example.org" });
  app.handleMessage({ type: "event", id: clipboard.id, event: "copied", value: "https://example.com" });
  await app.close();
  assert.deepEqual(errors, []);
  assert.deepEqual(seen, [
    ["toggle", true],
    ["save"],
    ["menu", "save-as"],
    ["group", "example.org"],
    ["submit", "example.org"],
    ["copied", "https://example.com"],
  ]);
});

test("the overlays are accepted and report their events", async () => {
  const app = await createApp(headless);
  const errors: HostError[] = [];
  const seen: unknown[] = [];
  app.on("error", (error) => errors.push(error));

  const alert = app
    .alertDialog({
      open: true,
      title: "Delete 3 files?",
      description: "This cannot be undone.",
      icon: "circle-alert",
      okText: "Delete",
      cancelText: "Cancel",
    })
    .on("ok", () => seen.push(["ok"]))
    .on("cancel", () => seen.push(["cancel"]));
  const sheet = app
    .sheet({ open: true, side: "left", title: "Filters", size: 320 })
    .on("close", () => seen.push(["close"]));
  const popover = app
    .popover({ open: false, anchor: "bottom-right", arrow: true })
    .on("change", ({ checked }) => seen.push(["popover", checked]));
  const card = app
    .hoverCard({ openDelay: 300, closeDelay: 200, anchor: "top-center" })
    .on("change", ({ checked }) => seen.push(["card", checked]));

  app.window({
    root: app.div({ class: "flex flex-col gap-2 p-4" }).append(
      alert.append(app.text({ content: "3 files" })),
      sheet.append(app.input({ value: "*.png" })),
      popover.append(app.button({ label: "Sort" }), app.text({ content: "By name" })),
      card.append(app.button({ label: "Info" }), app.text({ content: "Hovered" })),
    ),
  });
  app.commit();

  app.handleMessage({ type: "event", id: alert.id, event: "ok" });
  app.handleMessage({ type: "event", id: sheet.id, event: "close" });
  app.handleMessage({ type: "event", id: popover.id, event: "change", checked: true });
  app.handleMessage({ type: "event", id: card.id, event: "change", checked: false });
  await app.close();
  assert.deepEqual(errors, []);
  assert.deepEqual(seen, [["ok"], ["close"], ["popover", true], ["card", false]]);
});

test("an a11y prop is validated by the host", async () => {
  const app = await createApp(headless);
  const errors: HostError[] = [];
  app.on("error", (error) => errors.push(error));

  const ok = app.div({ a11y: { id: "save", role: "button", label: "Save", keyshortcuts: "ctrl-s" } });
  const bad = app.div({ a11y: { lable: "typo" } as never });
  // `bad` is not appended: the host rejects its `create`, so it does not exist there.
  app.window({ root: app.div().append(ok) });
  app.commit();
  ok.set({ a11y: null });
  app.commit();

  await app.close();
  assert.deepEqual(errors.map((error) => [error.code, error.op]), [["invalid_props", "create"]]);
});


test("shortcuts, keyDown, doubleClick and focus are accepted and dispatched", async () => {
  const app = await createApp(headless);
  const errors: HostError[] = [];
  app.on("error", (error) => errors.push(error));
  const seen: unknown[] = [];
  app.on("shortcut", (event) => seen.push(["shortcut", event]));
  const area = app
    .div()
    .on("keyDown", ({ key }) => seen.push(["key", key]))
    .on("doubleClick", () => seen.push(["double"]));
  const search = app.input().on("keyDown", ({ key }) => seen.push(["input", key]));
  const bar = app.menuBar({ menus: [{ label: "File", items: [{ id: "save", label: "Save", shortcut: "ctrl-s" }] }] });
  app.window({ root: app.div().append(area, search, bar) });
  app.shortcuts(["f5", "ctrl-shift-p"]);
  search.focus();
  app.commit();
  app.shortcuts(["ctrl-"]).commit();

  app.handleMessage({ type: "shortcut", keys: "f5", window: 1 });
  app.handleMessage({ type: "event", id: area.id, event: "keyDown", value: "escape" });
  app.handleMessage({ type: "event", id: area.id, event: "doubleClick" });
  app.handleMessage({ type: "event", id: search.id, event: "keyDown", value: "down" });
  await app.close();
  assert.deepEqual(errors.map((e) => [e.code, e.op]), [["invalid_json", "shortcuts"]]);
  assert.deepEqual(seen, [["shortcut", { keys: "f5", window: 1 }], ["key", "escape"], ["double"], ["input", "down"]]);
});

test("system actions get their results; drop delivers paths", async () => {
  const app = await createApp(headless);
  const errors: HostError[] = [];
  app.on("error", (error) => errors.push(error));
  const dropped: string[][] = [];
  const area = app.div().on("drop", ({ paths }) => dropped.push(paths));
  app.window({ root: area });
  app.commit();

  // Headless, dialogs and the clipboard answer null; several requests resolve independently.
  const [opened, saved, text] = await Promise.all([
    app.openDialog({ multiple: true, prompt: "Open" }),
    app.saveDialog({ name: "new.txt" }),
    app.readClipboard(),
  ]);
  assert.deepEqual([opened, saved, text], [null, null, null]);
  app.writeClipboard("hi");
  app.openUrl("https://example.com");
  app.handleMessage({ type: "event", id: area.id, event: "drop", paths: ["C:\\a.txt"] });
  // Rejected by the host: the promise fails with the host's error.
  await assert.rejects(app.openDialog({ files: false }), /host rejected 'openDialog'/);
  await app.close();
  await assert.rejects(app.readClipboard(), HostExitedError);
  assert.deepEqual(errors, []);
  assert.deepEqual(dropped, [["C:\\a.txt"]]);
});

test("Dialog, notify and tooltip are accepted; their events are dispatched", async () => {
  const app = await createApp(headless);
  const errors: HostError[] = [];
  app.on("error", (error) => errors.push(error));
  const seen: unknown[] = [];
  app.on("notification", (event) => seen.push(["notification", event]));
  const dialog = app
    .dialog({ open: true, title: "Rename", okText: "Save", cancelText: "Cancel", width: 400 })
    .append(app.input({ value: "notes.txt" }))
    .on("ok", () => seen.push(["ok"]))
    .on("cancel", () => seen.push(["cancel"]));
  app.window({ root: app.div({ tooltip: "Area" }).append(dialog, app.button({ label: "Save", tooltip: "Ctrl+S" })) });
  app.commit();
  app.notify({ message: "Saved", title: "Done", type: "success", id: "saved", autohide: false });
  app.notify({ message: "x", type: "fatal" as never });

  app.handleMessage({ type: "event", id: dialog.id, event: "ok" });
  app.handleMessage({ type: "event", id: dialog.id, event: "cancel" });
  app.handleMessage({ type: "notification", id: "saved", window: 1 });
  await app.close();
  assert.deepEqual(errors.map((e) => [e.code, e.op]), [["invalid_json", "notify"]]);
  assert.deepEqual(seen, [["ok"], ["cancel"], ["notification", { id: "saved", window: 1 }]]);
});

test("VirtualList, Table and Tabs are accepted; their events are dispatched", async () => {
  const app = await createApp(headless);
  const errors: HostError[] = [];
  app.on("error", (error) => errors.push(error));
  const seen: unknown[] = [];
  const list = app.virtualList({ rowHeight: 24, class: "h-64" });
  for (let i = 0; i < 1000; i++) list.append(app.text({ content: `Row ${i}` }));
  const table = app
    .table({
      columns: [{ key: "name", name: "Name", width: 200, sortable: true }, { key: "size", name: "Size", align: "right" }],
      rows: [{ name: "a", size: 1 }, { name: "b", size: null }],
      selected: 0,
      stripe: true,
    })
    .on("change", ({ value }) => seen.push(["change", value]))
    .on("doubleClick", ({ value }) => seen.push(["doubleClick", value]))
    .on("sort", (sort) => seen.push(["sort", sort]));
  const tabs = app
    .tabs({ tabs: [{ value: "list", label: "List" }, { value: "table", label: "Table" }], value: "list", variant: "underline" })
    .on("change", ({ value }) => seen.push(["tabs", value]));
  app.window({ root: app.div().append(tabs, list, table) });
  app.commit();

  app.handleMessage({ type: "event", id: table.id, event: "change", value: 1 });
  app.handleMessage({ type: "event", id: table.id, event: "change", value: null });
  app.handleMessage({ type: "event", id: table.id, event: "doubleClick", value: 1 });
  app.handleMessage({ type: "event", id: table.id, event: "sort", value: { column: "name", direction: "desc" } });
  app.handleMessage({ type: "event", id: tabs.id, event: "change", value: "table" });
  await app.close();
  assert.deepEqual(errors, []);
  assert.deepEqual(seen, [
    ["change", 1],
    ["change", null],
    ["doubleClick", 1],
    ["sort", { column: "name", direction: "desc" }],
    ["tabs", "table"],
  ]);
});

test("a Tabs built from Tab nodes commits; `change` still comes from the Tabs", async () => {
  const app = await createApp(headless);
  const errors: HostError[] = [];
  app.on("error", (error) => errors.push(error));
  const seen: unknown[] = [];
  const tabs = app.tabs({ value: "files", variant: "underline" }).on("change", ({ value }) => seen.push(value));
  const files = app.tab({ value: "files", a11y: { label: "Files" } });
  files.append(app.icon({ name: "folder", class: "size-4" }), app.text({ content: "Files" }), app.badge({ count: 3 }));
  tabs.append(files, app.tab({ value: "history", label: "History", disabled: true }));
  app.window({ root: app.div().append(tabs) });
  app.commit();

  app.handleMessage({ type: "event", id: tabs.id, event: "change", value: "files" });
  await app.close();
  assert.deepEqual(errors, []);
  assert.deepEqual(seen, ["files"]);
});

test("Tree, Diff and Carousel are accepted; their events are dispatched", async () => {
  const app = await createApp(headless);
  const errors: HostError[] = [];
  app.on("error", (error) => errors.push(error));
  const seen: unknown[] = [];
  const tree = app
    .tree({
      items: [
        { id: "src", label: "src", expanded: true, children: [{ id: "src/lib.rs", label: "lib.rs" }] },
        { id: "README.md", label: "README.md" },
      ],
      value: "src",
    })
    .on("change", ({ value }) => seen.push(["change", value]))
    .on("expand", ({ value }) => seen.push(["expand", value]))
    .on("collapse", ({ value }) => seen.push(["collapse", value]));
  const diff = app
    .diff({
      patch: "diff --git a/notes.txt b/notes.txt\n--- a/notes.txt\n+++ b/notes.txt\n@@ -1,3 +1,4 @@\n one\n-two\n+2\n three\n+four\n",
      mode: "split",
    })
    .on("change", ({ value }) => seen.push(["diff", value]));
  const carousel = app
    .carousel({ value: 0, pagination: true, class: "h-40" })
    .append(app.text({ content: "One" }), app.text({ content: "Two" }))
    .on("change", ({ value }) => seen.push(["carousel", value]));
  app.window({ root: app.div().append(tree, diff, carousel) });
  app.commit();

  const selection = { path: "notes.txt", side: "modified", start: 1, end: 2, endSide: "modified" };
  app.handleMessage({ type: "event", id: tree.id, event: "change", value: "README.md" });
  app.handleMessage({ type: "event", id: tree.id, event: "expand", value: "src" });
  app.handleMessage({ type: "event", id: tree.id, event: "collapse", value: "src" });
  app.handleMessage({ type: "event", id: diff.id, event: "change", value: selection });
  app.handleMessage({ type: "event", id: diff.id, event: "change", value: null });
  app.handleMessage({ type: "event", id: carousel.id, event: "change", value: 1 });
  await app.close();
  assert.deepEqual(errors, []);
  assert.deepEqual(seen, [
    ["change", "README.md"],
    ["expand", "src"],
    ["collapse", "src"],
    ["diff", selection],
    ["diff", null],
    ["carousel", 1],
  ]);
});

test("Sidebar and Chart are accepted; the sidebar reports the chosen entry", async () => {
  const app = await createApp(headless);
  const errors: HostError[] = [];
  app.on("error", (error) => errors.push(error));
  const seen: unknown[] = [];
  const sidebar = app
    .sidebar({
      items: [
        {
          label: "General",
          items: [
            { id: "home", label: "Home", icon: "layout-dashboard" },
            { id: "inbox", label: "Inbox", icon: "inbox" },
          ],
        },
        { items: [{ id: "settings", label: "Settings", children: [{ id: "theme", label: "Theme" }] }] },
      ],
      value: "home",
    })
    .on("change", ({ value }) => seen.push(["sidebar", value]));
  const line = app.chart({ chart: "line", data: [{ label: "Mon", value: 12 }, { label: "Tue", value: 18 }], yAxis: true });
  const pie = app.chart({ chart: "pie", data: [{ label: "Rust", value: 60 }], innerRadius: 20 });
  app.window({ root: app.div().append(sidebar, line, pie) });
  app.commit();

  app.handleMessage({ type: "event", id: sidebar.id, event: "change", value: "theme" });
  await app.close();
  assert.deepEqual(errors, []);
  assert.deepEqual(seen, [["sidebar", "theme"]]);
});

test("malformed sidebar and chart props are invalid_props, and the host keeps running", async () => {
  const app = await createApp(headless);
  const errors: HostError[] = [];
  app.on("error", (error) => errors.push(error));

  app.sidebar({ items: [{ items: [{ id: "home" }] }] });
  app.chart({ data: [{ label: "Mon" }] });
  app.window({ root: app.div() });
  app.commit();
  await app.close();
  assert.deepEqual(
    errors.map((error) => error.code),
    ["invalid_props", "invalid_props"],
  );
});

test("a host crash is reported clearly", async () => {
  const app = await createApp(headless);
  const exited = new Promise<HostExit>((resolve) => app.on("exit", resolve));
  (app as unknown as { host: { kill(): void } }).host.kill();

  const exit = await exited;
  assert.notEqual(exit.code, 0);
  assert.throws(() => app.commit(), (err: unknown) => err instanceof HostExitedError && /host exited/.test(err.message));
});

test("window events are opt-in: the window subscribes and both the window and the app receive them", async () => {
  const app = await createApp(headless);
  const errors: HostError[] = [];
  app.on("error", (error) => errors.push(error));

  app.window({ root: app.div(), events: ["resize"] });
  app.commit();
  const settings = app.createWindow({ root: app.div() });
  const resized: number[] = [];
  settings.on("resize", (payload) => resized.push(payload.width ?? 0));
  app.commit();

  const all: string[] = [];
  app.on("windowEvent", (payload) => all.push(`${payload.id}:${payload.event}:${payload.width ?? "-"}`));
  app.handleMessage({ type: "windowEvent", id: settings.id, event: "resize", x: 10, y: 20, width: 300, height: 200 });
  app.handleMessage({ type: "windowEvent", id: 1, event: "resize", x: 0, y: 0, width: 420, height: 320 });
  // An event this window did not subscribe to still reaches the app-level listener.
  app.handleMessage({ type: "windowEvent", id: settings.id, event: "focus" });

  await app.close();
  assert.deepEqual(errors, []);
  assert.deepEqual(resized, [300]);
  assert.deepEqual(all, [`${settings.id}:resize:300`, "1:resize:420", `${settings.id}:focus:-`]);
});

test("a missing host binary fails with a helpful message", async () => {
  await assert.rejects(createApp({ hostPath: "does-not-exist.exe" }), /host not found at does-not-exist\.exe/);
});

test("defaultHostPath prefers the packaged host, then the build in this repository", () => {
  const exe = process.platform === "win32" ? "gpui-bridge-host.exe" : "gpui-bridge-host";
  // The platform package (`@indios/gpui-bridge-<platform>-<arch>`) carries the asset; a checkout has
  // none until it is staged, so this asserts whichever of the two applies and the order.
  const packaged = join(import.meta.dirname, "..", "..", `host-${process.platform}-${process.arch}`, "bin", exe);
  const saved = process.env.GPUI_BRIDGE_HOST;
  delete process.env.GPUI_BRIDGE_HOST;
  try {
    if (existsSync(packaged)) assert.equal(defaultHostPath(), packaged);
    else assert.match(defaultHostPath(), /[\\/]target[\\/](debug|release)[\\/]gpui-bridge-host(\.exe)?$/);
    process.env.GPUI_BRIDGE_HOST = "an-explicit-host";
    assert.equal(defaultHostPath(), "an-explicit-host");
  } finally {
    if (saved === undefined) delete process.env.GPUI_BRIDGE_HOST;
    else process.env.GPUI_BRIDGE_HOST = saved;
  }
});

test("createWindow gives each window its own id, root and closed event", async () => {
  const app = await createApp(headless);
  const errors: HostError[] = [];
  app.on("error", (error) => errors.push(error));

  const main = app.div();
  const other = app.div();
  app.window({ title: "Main", root: main });
  const settings = app.createWindow({ title: "Settings", root: other });
  const third = app.createWindow();
  assert.deepEqual([settings.id, third.id], [2, 3]);
  // A node can be the root of one window only.
  third.set({ root: main });
  app.commit();

  const closed: number[] = [];
  settings.on("closed", () => closed.push(settings.id));
  app.on("windowClosed", ({ id }) => closed.push(id * 10));
  app.handleMessage({ type: "windowClosed", id: 2 });
  assert.deepEqual(closed, [2, 20]);

  settings.close();
  app.commit();
  await app.close();
  // Only the shared root fails (the windowClosed above was injected, so the host still had window 2 to close).
  assert.deepEqual(
    errors.map((error) => [error.code, error.op]),
    [["invalid_operation", "window"]],
  );
});
