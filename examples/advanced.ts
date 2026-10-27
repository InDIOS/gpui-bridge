// The data components of the first batch: a Tree, a Diff and a Carousel. Selection, expansion
// and slides are reported with events; `a11y` names the parts that have no visible text.
// Run: node examples/advanced.ts   (after `npm run build` in client-node/ and `cargo build` in host/)

import { createApp, type DiffSelection, type TreeNode } from "gpui-bridge";

const app = await createApp({ debug: process.argv.includes("--debug") });

const status = app.text({ content: "Pick an item", class: "text-sm text-muted-foreground" });
const say = (content: string) => {
  status.set({ content });
  app.commit();
};

const treeItems: TreeNode[] = [
  {
    id: "src",
    label: "src",
    expanded: true,
    children: [
      { id: "src/main.rs", label: "main.rs" },
      { id: "src/ui", label: "ui", children: [{ id: "src/ui/tree.rs", label: "tree.rs" }] },
    ],
  },
  { id: "README.md", label: "README.md" },
  { id: "target", label: "target", disabled: true },
];

const tree = app
  .tree({ items: treeItems, value: "src", a11y: { role: "tree", label: "Repository" }, class: "h-64 overflow-hidden border border-border rounded-md" })
  .on("change", ({ value }) => say(`Tree: selected ${value}`))
  .on("expand", ({ value }) => say(`Tree: expanded ${value}`))
  .on("collapse", ({ value }) => say(`Tree: collapsed ${value}`));

const patch = `diff --git a/notes.txt b/notes.txt
--- a/notes.txt
+++ b/notes.txt
@@ -1,3 +1,4 @@
 one
-two
+2
 three
+four
`;

const diff = app
  .diff({ patch, mode: "unified", softWrap: true, a11y: { label: "Patch" }, class: "h-80 overflow-hidden border border-border rounded-md" })
  .on("change", ({ value }: { value: DiffSelection | null }) =>
    say(value ? `Diff: ${value.path} ${value.side} ${value.start}-${value.end}` : "Diff: no selection"),
  );

const slide = (title: string, text: string) =>
  app
    .div({ class: "flex flex-col items-center justify-center size-full gap-2 bg-muted rounded-md", a11y: { role: "group", label: title } })
    .append(app.text({ content: title, class: "text-lg font-medium" }), app.text({ content: text, class: "text-sm text-muted-foreground" }));

const carousel = app
  .carousel({ value: 0, pagination: true, class: "h-56" })
  .append(
    slide("Tree", "Items are data: one prop, nested children."),
    slide("Diff", "A patch is parsed in the host; the selection is data too."),
    slide("Carousel", "Each child is a slide; the host keeps the index."),
  )
  .on("change", ({ value }) => say(`Carousel: slide ${value + 1}`));

const root = app
  .div({ class: "flex flex-col size-full gap-4 p-4 bg-background text-foreground" })
  .append(
    app.text({ content: "Tree", class: "text-sm font-medium" }),
    tree,
    app.text({ content: "Diff", class: "text-sm font-medium" }),
    diff,
    app.text({ content: "Carousel", class: "text-sm font-medium" }),
    carousel,
    status,
  );
app.window({ title: "Advanced components", width: 860, height: 900, root });
app.commit();
