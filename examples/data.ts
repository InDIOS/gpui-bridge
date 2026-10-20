// Big data: a VirtualList and a Table of 10,000 rows each, switched with Tabs. Only the rows in
// view are drawn. The Table sorts by name or size (the app sorts and sends `rows` again).
// Run: node examples/data.ts   (after `npm run build` in client-node/ and `cargo build` in host/)

import { createApp, type TableCell, type TableSort } from "gpui-bridge";

const app = await createApp({ debug: process.argv.includes("--debug") });
const COUNT = 10_000;

const status = app.text({ content: "Click a row", class: "text-sm text-muted-foreground" });
const say = (content: string) => {
  status.set({ content });
  app.commit();
};

// List: every row is an ordinary node with its own events.
const list = app.virtualList({ rowHeight: 28, class: "flex-1 border border-border rounded-md" });
for (let i = 1; i <= COUNT; i++) {
  list.append(
    app
      .div({ class: "flex items-center gap-2 px-3 border-b border-border transition duration-300 hover:bg-accent cursor-pointer" })
      .append(app.icon({ name: "file", class: "size-4 text-muted-foreground" }), app.text({ content: `Item ${i}`, class: "text-sm" }))
      .on("click", () => say(`List: item ${i}`)),
  );
}

// Table: plain data in props.
const original: Record<string, TableCell>[] = Array.from({ length: COUNT }, (_, i) => ({
  id: i + 1,
  name: `file-${String((i * 7919) % COUNT).padStart(5, "0")}.txt`,
  size: (i * 104729) % 50_000,
  kind: ["text", "image", "audio", "video"][i % 4],
}));
let rows = original;
const table = app
  .table({
    columns: [
      { key: "id", name: "#", width: 70, align: "right" },
      { key: "name", name: "Name", width: 220, sortable: true },
      { key: "size", name: "Size (bytes)", width: 140, align: "right", sortable: true },
      { key: "kind", name: "Kind", width: 120 },
    ],
    rows,
    stripe: true,
    class: "flex-1",
  })
  .on("change", ({ value }) => say(value === null ? "Table: no selection" : `Table: row ${value} (${rows[value].name})`))
  .on("doubleClick", ({ value }) => say(`Table: opened ${rows[value].name}`))
  .on("sort", ({ column, direction }: TableSort) => {
    const sign = direction === "asc" ? 1 : -1;
    rows = direction === "none" ? original : [...original].sort((a, b) => (a[column]! < b[column]! ? -sign : a[column]! > b[column]! ? sign : 0));
    // Indexes change with the order: drop the selection.
    table.set({ rows, selected: null });
    say(`Table: sorted by ${column} ${direction}`);
  });

// Tabs: the app shows the chosen panel by moving it in; the other waits in a detached div.
const panel = app.div({ class: "flex flex-col flex-1 min-h-0" }).append(list);
const parked = app.div().append(table);
const panels = { list, table };
const tabs = app
  .tabs({
    tabs: [
      { value: "list", label: "VirtualList" },
      { value: "table", label: "Table" },
    ],
    value: "list",
    variant: "underline",
  })
  .on("change", ({ value }) => {
    const shown = value as keyof typeof panels;
    parked.append(panels[shown === "list" ? "table" : "list"]);
    panel.append(panels[shown]);
    say(`Showing the ${shown}`);
  });

const root = app.div({ class: "flex flex-col size-full gap-3 p-4 bg-background text-foreground" }).append(tabs, panel, status);
app.window({ title: "Data", width: 720, height: 560, root });
app.commit();
