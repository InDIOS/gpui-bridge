// Menus: a MenuBar and a right-click context menu. Each choice shows in the status line;
// "Word wrap" is a checked item the app toggles by sending the menus again.
// With --frameless the MenuBar sits inside the app's own title bar (a `drag` area).
// Run: node examples/menus.ts [--frameless]   (after `npm run build` in client-node/ and `cargo build` in host/)

import { createApp, type Menu } from "gpui-bridge";

const frameless = process.argv.includes("--frameless");
const app = await createApp({ debug: process.argv.includes("--debug") });

let wrap = true;
const menus = (): Menu[] => [
  {
    label: "File",
    items: [
      { id: "new", label: "New", icon: "plus" },
      { id: "open", label: "Open…", icon: "folder-open" },
      { label: "Open recent", items: [{ id: "recent:notes.txt", label: "notes.txt" }, { id: "recent:todo.md", label: "todo.md" }] },
      { separator: true },
      { id: "quit", label: "Quit" },
    ],
  },
  {
    label: "Edit",
    items: [
      { id: "undo", label: "Undo", icon: "undo", disabled: true },
      { id: "copy", label: "Copy", icon: "copy" },
    ],
  },
  { label: "View", items: [{ id: "wrap", label: "Word wrap", checked: wrap }] },
];

const root = app.div({ class: "flex flex-col size-full bg-background text-foreground" });
const bar = app.menuBar({ menus: menus(), class: "px-1" });
const status = app.text({ content: "Choose a menu item, or right-click the box.", class: "text-sm text-muted-foreground" });
const area = app.div({
  class: "flex items-center justify-center w-80 h-40 rounded-lg border border-border bg-muted",
  contextMenu: [
    { id: "cut", label: "Cut" },
    { id: "copy", label: "Copy", icon: "copy" },
    { id: "paste", label: "Paste", disabled: true },
    { separator: true },
    { label: "Sort by", items: [{ id: "sort:name", label: "Name" }, { id: "sort:date", label: "Date" }] },
  ],
});
area.append(app.text({ content: "Right-click here", class: "text-sm" }));

const onMenu = (source: string) => ({ item }: { item: string }) => {
  if (item === "quit") return void app.close();
  if (item === "wrap") {
    wrap = !wrap;
    bar.set({ menus: menus() });
  }
  status.set({ content: `${source}: ${item}${item === "wrap" ? ` (now ${wrap ? "on" : "off"})` : ""}` });
  app.commit();
};
bar.on("menu", onMenu("menu bar"));
area.on("menu", onMenu("context menu"));

const body = app.div({ class: "flex flex-col flex-1 items-center justify-center gap-4" });
body.append(area, status);

if (frameless) {
  const titleBar = app.div({ class: "flex flex-row items-center h-9 shrink-0 bg-muted border-b border-border" });
  const drag = app.div({ class: "flex flex-row flex-1 items-center gap-2 h-full px-2", windowControl: "drag" });
  drag.append(app.icon({ name: "menu", class: "size-4 text-primary" }), bar);
  const close = app.div({ class: "flex items-center justify-center w-12 h-full hover:bg-danger", windowControl: "close" });
  close.append(app.icon({ name: "window-close", class: "size-4" }));
  titleBar.append(drag, close);
  root.append(titleBar, body);
} else {
  root.append(app.div({ class: "border-b border-border" }).append(bar), body);
}

app.on("windowClosed", () => console.log("windowClosed"));
app.window({ title: "Menus", width: 560, height: 380, frameless, root });
app.commit();
