// Command: a palette inline and the same palette in a Dialog; confirming an entry shows its id.
// Run: node examples/command.ts   (after `npm run build` in client-node/ and `cargo build` in host/)

import { createApp, type CommandSection } from "gpui-bridge";

const app = await createApp({ debug: process.argv.includes("--debug") });

const items: CommandSection[] = [
  {
    label: "File",
    items: [
      { id: "open", label: "Open file", icon: "folder", keywords: ["load"] },
      { id: "save", label: "Save" },
      { id: "share", label: "Share (soon)", disabled: true },
    ],
  },
  {
    label: "View",
    items: [
      { id: "dark", label: "Dark theme", icon: "moon", checked: true },
      { id: "sidebar", label: "Toggle sidebar", icon: "panel-left", keywords: ["navigation"] },
    ],
  },
  { items: [{ id: "quit", label: "Quit" }] },
];

const root = app.div({ class: "flex flex-col gap-3 p-4 size-full bg-background" });
const label = app.text({ content: "Type to filter, Enter to confirm", class: "text-sm text-muted-foreground" });
const inline = app.command({ items, placeholder: "Type a command or search..." });
const open = app.button({ label: "Open palette in a dialog", variant: "secondary" });
const palette = app.command({ items, placeholder: "Search..." });
const dialog = app.dialog({ open: false, title: "Command palette", okText: "Close", width: 420 }).append(palette);

function chosen(where: string, id: string) {
  label.set({ content: `${where}: ${id}` });
  if (id === "quit") return app.close();
  app.commit();
}
inline.on("confirm", ({ value }) => chosen("inline", value));
palette.on("confirm", ({ value }) => {
  dialog.set({ open: false });
  chosen("dialog", value);
});
open.on("click", () => {
  dialog.set({ open: true });
  app.commit();
});

root.append(label, inline, open, dialog);
app.window({ title: "Command", width: 480, height: 520, root });
app.commit();
