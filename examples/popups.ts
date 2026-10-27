// The four overlays of M13 in one window: a confirmation that opens on top of a dialog, a panel of
// filters, a menu anchored to its trigger and a card on hover. Everything reports its events in the
// status line, so a run through them says what reached the client.
// Run: pnpm dev popups   (after `pnpm bootstrap`)

import { createApp } from "gpui-bridge";

const app = await createApp({ debug: process.argv.includes("--debug") });

const root = app.div({ class: "flex flex-col size-full gap-4 p-4 bg-background text-foreground" });

const title = app.label({ content: "Overlays", secondary: "(M13)", highlight: "M13" });
const status = app.label({ content: "Nothing happened yet", secondary: "" });
root.append(app.div({ class: "flex items-center gap-3" }).append(title, status));

function report(text: string) {
  status.set({ content: text });
  app.commit();
}

// A dialog with an Input, and the confirmation that opens on top of it: the alert wins the layer,
// and closing it brings the dialog back with its text as it was (the two share one layer per
// window, PROTOCOL.md section 24).
let note = "Remember the milk";
let asking: "note" | "files" = "files";

const noteField = app.input({ value: note });
noteField.on("change", ({ value }) => (note = value));

const dialog = app
  .dialog({ open: false, title: "Edit note", okText: "Save", cancelText: "Close", width: 380 })
  .append(
    app.div({ class: "flex flex-col gap-3" }).append(
      noteField,
      app.button({ label: "Delete note", variant: "danger" }).on("click", () => ask("note")),
    ),
  );
dialog.on("ok", () => report(`Note saved as "${note}"`));

const alert = app
  .alertDialog({ open: false, icon: "circle-alert", okText: "Delete", cancelText: "Cancel" })
  .on("ok", () => {
    if (asking === "note") {
      dialog.set({ open: false });
      report("Note deleted");
    } else {
      report("3 files deleted");
    }
  })
  .on("cancel", () => report("Cancelled"));

function ask(what: "note" | "files") {
  asking = what;
  alert.set(
    what === "note"
      ? { open: true, title: "Delete this note?", description: "It is gone for good." }
      : { open: true, title: "Delete 3 files?", description: "This cannot be undone." },
  );
  app.commit();
}

const editNote = app.button({ label: "Edit note...", variant: "primary" }).on("click", () => {
  dialog.set({ open: true });
  app.commit();
});
const deleteFiles = app.button({ label: "Delete 3 files..." }).on("click", () => ask("files"));
root.append(app.div({ class: "flex items-center gap-2" }).append(editNote, deleteFiles));

// A panel that slides in from either side; its X, Escape and a click on the dimmed overlay all
// report `close`. An Input inside it edits as anywhere else in the window.
const search = app.input({ value: "", placeholder: "*.png" });
const sheet = app
  .sheet({ open: false, side: "right", title: "Filters", size: 340 })
  .on("close", () => report("Filters closed"))
  .append(
    app.div({ class: "flex flex-col gap-3" }).append(
      app.label({ content: "File name", secondary: "" }),
      search,
      app.div({ class: "flex flex-row gap-2" }).append(
        app.button({ label: "Apply", variant: "primary" }).on("click", () => {
          sheet.set({ open: false });
          app.commit();
          report("Filters applied");
        }),
        app.button({ label: "Clear" }).on("click", () => {
          search.set({ value: "" });
          app.commit();
          report("Filters cleared");
        }),
      ),
    ),
  );

function openFilters(side: "left" | "right") {
  sheet.set({ open: true, side });
  app.commit();
}
root.append(
  app.div({ class: "flex items-center gap-2" }).append(
    app.button({ label: "Filters from the right" }).on("click", () => openFilters("right")),
    app.button({ label: "From the left" }).on("click", () => openFilters("left")),
  ),
);

// The first child of a `Popover` is its trigger; the rest is the panel. `class` styles the
// trigger's slot, so `w-full` fills the box around it (the panel itself is styled by its children).
// A click on the trigger reports its own `click` and toggles the panel; Escape and a click outside
// close it, and `change` says which way it went.
const sort = app
  .popover({ anchor: "bottom-left", arrow: true, class: "w-full" })
  .on("change", ({ checked }) => report(`Sort menu ${checked ? "open" : "closed"}`));
sort.append(
  app.button({ label: "Sort by..." }).on("click", () => report("Sort clicked")),
  app.div({ class: "flex flex-col gap-1" }).append(
    app.button({ label: "Name" }).on("click", () => choose("name")),
    app.button({ label: "Size" }).on("click", () => choose("size")),
  ),
);

function choose(by: string) {
  sort.set({ open: false });
  app.commit();
  report(`Sorted by ${by}`);
}
root.append(app.div({ class: "flex flex-col w-56 gap-1" }).append(sort));

// A card on hover: the mouse is the only way in, and its `change` says when it came and went.
const card = app
  .hoverCard({ openDelay: 300, closeDelay: 200, anchor: "top-center" })
  .on("change", ({ checked }) => report(`Card ${checked ? "shown" : "hidden"}`));
card.append(
  app.button({ label: "Hover me" }),
  app.div({ class: "flex flex-col gap-1" }).append(
    app.label({ content: "Hover card", secondary: "" }),
    app.text({ content: "Only the mouse reaches this", class: "text-xs text-muted-foreground" }),
  ),
);
root.append(card);

root.append(
  app.text({
    content:
      "Tab and Escape reach the dialog, the panel and the popover; the card and the popover's panel are reached with the mouse.",
    class: "text-sm text-muted-foreground",
  }),
  dialog,
  sheet,
  alert,
);

app.window({ title: "Overlays", width: 720, height: 600, root });
app.commit();
