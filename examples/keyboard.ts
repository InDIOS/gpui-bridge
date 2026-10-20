// Keyboard: menu shortcuts (Ctrl+N, Ctrl+F), an app shortcut (F5), a list moved with the
// arrow keys (keyDown on a focusable div) and opened with Enter or a double click, and a
// search box that Escape clears.
// Run: node examples/keyboard.ts   (after `npm run build` in client-node/ and `cargo build` in host/)

import { createApp } from "gpui-bridge";

const app = await createApp({ debug: process.argv.includes("--debug") });

const items = ["Apples", "Bananas", "Cherries", "Dates", "Elderberries"];
let selected = 0;
const status = app.text({ content: "Click the list, then use ↑ ↓ and Enter.", class: "text-sm text-muted-foreground" });
const say = (content: string) => status.set({ content });

const bar = app.menuBar({
  menus: [
    { label: "File", items: [{ id: "new", label: "New", shortcut: "ctrl-n" }, { id: "quit", label: "Quit", shortcut: "ctrl-q" }] },
    { label: "Edit", items: [{ id: "find", label: "Find", shortcut: "ctrl-f" }, { id: "reload", label: "Reload", shortcut: "f5" }] },
  ],
});

const search = app.input({ placeholder: "Search (Ctrl+F; Escape clears)", class: "w-72" });
search.on("keyDown", ({ key }) => {
  if (key !== "escape") return;
  search.set({ value: "" });
  say("Search cleared");
  app.commit();
});

const rows = items.map((name, ix) =>
  app
    .div({ class: "px-3 py-1 rounded-md" })
    .append(app.text({ content: name, class: "text-sm" }))
    .on("click", () => select(ix))
    .on("doubleClick", () => open(ix)),
);
const list = app.div({ class: "flex flex-col w-72 p-1 border border-border rounded-lg" }).append(...rows);

function paint() {
  rows.forEach((row, ix) => row.set({ class: `px-3 py-1 rounded-md ${ix === selected ? "bg-accent text-accent-foreground" : ""}` }));
}
function select(ix: number) {
  selected = Math.max(0, Math.min(items.length - 1, ix));
  paint();
  say(`Selected ${items[selected]}`);
  app.commit();
}
function open(ix: number) {
  say(`Opened ${items[ix]}`);
  app.commit();
}

list.on("keyDown", ({ key }) => {
  if (key === "down") select(selected + 1);
  else if (key === "up") select(selected - 1);
  else if (key === "enter") open(selected);
});

bar.on("menu", ({ item }) => {
  if (item === "quit") return void app.close();
  if (item === "find") search.focus();
  if (item === "new") say("New (Ctrl+N)");
  if (item === "reload") say("Reloaded (F5, from the menu)");
  app.commit();
});
app.shortcuts(["ctrl-shift-d"]);
app.on("shortcut", ({ keys }) => {
  say(`App shortcut ${keys}`);
  app.commit();
});

const root = app.div({ class: "flex flex-col size-full bg-background text-foreground" });
const body = app.div({ class: "flex flex-col gap-4 p-6" }).append(search, list, status);
root.append(app.div({ class: "border-b border-border" }).append(bar), body);
paint();
app.window({ title: "Keyboard", width: 520, height: 420, root });
app.commit();
