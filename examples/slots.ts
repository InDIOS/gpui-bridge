// Slots as children (M18): an Accordion whose sections hold controls (AccordionItem), a Dialog and
// an AlertDialog with their own footer buttons (DialogFooter), and a StatusBar whose sides are
// nodes (StatusBarLeft, StatusBarRight). The status bar shows what the host reports.
// Run: pnpm dev slots   (after `pnpm bootstrap`)

import { createApp } from "gpui-bridge";

const app = await createApp({ debug: process.argv.includes("--debug") });

const status = app.text({ content: "Ready" });
const report = (line: string) => {
  console.log(line);
  status.set({ content: line });
  app.commit();
};

const accordion = app
  .accordion({ multiple: true })
  .on("change", ({ value }) => report(`section ${value}`))
  .append(
    app.accordionItem({ id: "general", title: "General", open: true, class: "flex flex-col gap-2" }).append(
      app.switch({ label: "Start with the system" }).on("change", ({ checked }) => report(`start: ${checked}`)),
      app.switch({ label: "Check for updates", checked: true }).on("change", ({ checked }) => report(`updates: ${checked}`)),
    ),
    app.accordionItem({ id: "file", title: "File" }).append(
      app.div({ class: "flex gap-2" }).append(
        app.button({ label: "Rename…" }).on("click", () => {
          rename.set({ open: true });
          app.commit();
        }),
        app.button({ label: "Delete…", variant: "danger" }).on("click", () => {
          remove.set({ open: true });
          app.commit();
        }),
      ),
    ),
    app.accordionItem({ id: "locked", title: "Locked", disabled: true }),
  );

// The footer's buttons send their own `click`; the app closes the dialog with `open: false`.
const close = (dialog: { set(props: { open: boolean }): unknown }, line: string) => {
  dialog.set({ open: false });
  report(line);
};
const name = app.input({ value: "notes.txt" });
const rename = app
  .dialog({ title: "Rename", width: 420 })
  .on("cancel", () => report("rename: cancel"))
  .append(
    name,
    app.dialogFooter({ class: "gap-2" }).append(
      app.button({ label: "Discard" }).on("click", () => close(rename, "rename: discard")),
      app.button({ label: "Save copy" }).on("click", () => close(rename, "rename: save copy")),
      app.button({ label: "Save", variant: "primary" }).on("click", () => close(rename, "rename: save")),
    ),
  );
const remove = app
  .alertDialog({ title: "Delete notes.txt?", description: "This cannot be undone.", icon: "circle-alert" })
  .on("cancel", () => report("delete: cancel"))
  .append(
    app.dialogFooter({ class: "gap-2" }).append(
      app.button({ label: "Keep" }).on("click", () => close(remove, "delete: keep")),
      app.button({ label: "Delete", variant: "danger" }).on("click", () => close(remove, "delete: done")),
    ),
  );

const bar = app.statusBar({ class: "border-t" }).append(
  app.statusBarLeft({ class: "gap-1" }).append(app.icon({ name: "check" }), status),
  app.statusBarRight().append(
    app.button({ label: "UTF-8", variant: "ghost" }).on("click", () => report("encoding")),
  ),
);

app.window({
  title: "Slots",
  width: 640,
  height: 480,
  root: app.div({ class: "size-full flex flex-col" }).append(
    app.div({ class: "flex-1 p-4" }).append(accordion),
    bar,
    rename,
    remove,
  ),
});
app.commit();
