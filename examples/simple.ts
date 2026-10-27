// The seven simple components of M12 in one window: a toggle, a split button with a menu, a rich
// label, an empty state with its action, loading text, a framed text field and a copy button.
// Run: pnpm dev simple   (after `pnpm bootstrap`)

import { createApp } from "gpui-bridge";

const app = await createApp({ debug: process.argv.includes("--debug") });

const root = app.div({ class: "flex flex-col size-full gap-4 p-4 bg-background" });

// A title, and the loading text next to it.
const title = app.label({ content: "Simple components", secondary: "(M12)", highlight: "M12" });
const loading = app.shimmer({ content: "Loading…", duration: 2.5, spread: 0.4 });
root.append(app.div({ class: "flex items-center gap-3" }).append(title, loading));

// A toggle and a split button with a menu.
const bold = app.toggle({ label: "Bold", icon: "asterisk", variant: "outline", tooltip: "Bold text" });
const save = app.dropdownButton({
  label: "Save",
  icon: "check",
  variant: "primary",
  menuLabel: "Save options",
  items: [
    { id: "save-as", label: "Save as…", shortcut: "ctrl-s" },
    { id: "copy-link", label: "Copy link", icon: "copy" },
    { separator: true },
    { id: "discard", label: "Discard", disabled: true },
  ],
});
root.append(app.div({ class: "flex items-center gap-2" }).append(bold, save));

// One field with a prefix and a suffix icon, and the copy button for its address.
let address = "https://example.com/docs";
const field = app.inputGroup({
  value: "example.com",
  placeholder: "host",
  prefix: "https://",
  suffixIcon: "check",
  a11y: { label: "Address" },
  class: "flex-1",
});
const copy = app.clipboard({
  value: address,
  tooltip: "Copy the address",
  a11y: { label: "Copy the address" },
});
root.append(app.div({ class: "flex items-center gap-2" }).append(field, copy));

// The empty state goes away with the first file; its button creates the labels.
const files = app.div({ class: "flex flex-col gap-1" });
const choose = app.button({ label: "Choose a file", variant: "secondary" });
const empty = app
  .empty({
    icon: "folder-open",
    title: "No files",
    description: "Choose one, or copy the address above.",
    a11y: { role: "group", label: "No files" },
    class: "h-48",
  })
  .append(choose);
root.append(empty, files);

// What the user did, in the last line.
const status = app.label({ content: "Nothing happened yet", secondary: "" });
root.append(status);

function report(text: string) {
  status.set({ content: text });
  app.commit();
}

bold.on("change", ({ checked }) => report(`Bold: ${checked ? "on" : "off"}`));
save.on("click", () => report("Save clicked"));
save.on("menu", ({ item }) => report(`Save menu: ${item}`));
field.on("submit", ({ value }) => {
  address = `https://${value}`;
  copy.set({ value: address });
  report(`Address: ${address}`);
});
copy.on("copied", ({ value }) => report(`Copied ${value}`));

let chosen = 0;
choose.on("click", () => {
  chosen += 1;
  empty.remove();
  files.append(app.label({ content: `file-${chosen}.txt`, secondary: "chosen" }));
  report(`Chose file-${chosen}.txt`);
});

app.window({ title: "Simple components", width: 640, height: 560, root });
app.commit();