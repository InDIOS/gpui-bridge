// Overlays: a Rename dialog with an Input inside, toasts of each type (one clickable, one
// also sent to the Windows notification center), and tooltips on a div and a Button.
// Run: node examples/overlays.ts   (after `npm run build` in client-node/ and `cargo build` in host/)

import { createApp } from "gpui-bridge";

const app = await createApp({ debug: process.argv.includes("--debug") });

let fileName = "notes.txt";
const current = app.text({ content: `File: ${fileName}`, class: "text-sm" });

const nameField = app.input({ value: fileName });
// A transition inside the dialog body: its hover animates like anywhere else in the window.
const swatch = app
  .div({ class: "flex items-center justify-center h-10 rounded-md border border-border transition duration-500 hover:bg-accent" })
  .append(app.text({ content: "Hover inside the dialog", class: "text-sm" }));
const rename = app
  .dialog({ open: false, title: "Rename file", okText: "Rename", cancelText: "Cancel", width: 380 })
  .append(app.text({ content: "New name:", class: "text-sm text-muted-foreground" }), nameField, swatch);
let typed = fileName;
nameField.on("change", ({ value }) => (typed = value));
nameField.on("submit", () => answer(true));
rename.on("ok", () => answer(true)).on("cancel", () => answer(false));

function answer(ok: boolean) {
  if (ok && typed.trim()) {
    fileName = typed.trim();
    current.set({ content: `File: ${fileName}` });
    app.notify({ message: `Renamed to ${fileName}`, type: "success", id: "renamed" });
  }
  rename.set({ open: false }); // already closed after ok/cancel; needed after Enter in the field
  app.commit();
}

const open = app.button({ label: "Rename…", variant: "primary", tooltip: "Opens a dialog" }).on("click", () => {
  typed = fileName;
  nameField.set({ value: fileName });
  rename.set({ open: true });
  nameField.focus();
  app.commit();
});

const toasts = app.div({ class: "flex flex-row flex-wrap gap-2" });
for (const type of ["info", "success", "warning", "error"] as const) {
  toasts.append(app.button({ label: type }).on("click", () => app.notify({ message: `A ${type} toast`, title: type, type })));
}
toasts.append(
  app.button({ label: "Clickable" }).on("click", () => app.notify({ message: "Click me", id: "clickable", autohide: false })),
  app.button({ label: "System too", tooltip: "Also in the Windows notification center" }).on("click", () =>
    app.notify({ message: "Sent to Windows too", title: "gpui-bridge", system: true }),
  ),
);
app.on("notification", ({ id }) => {
  current.set({ content: `Clicked the "${id}" toast` });
  app.commit();
});

const hint = app
  .div({ class: "flex items-center justify-center w-48 h-16 rounded-lg border border-border", tooltip: "A div with a tooltip" })
  .append(app.text({ content: "Hover me", class: "text-sm" }));

const root = app.div({ class: "flex flex-col size-full gap-4 p-6 bg-background text-foreground" });
root.append(app.div({ class: "flex flex-row items-center gap-3" }).append(open, current), toasts, hint, rename);
app.window({ title: "Overlays", width: 620, height: 420, root });
app.commit();
