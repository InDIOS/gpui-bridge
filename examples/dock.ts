// Dock (M16): panels the user rearranges. Drag a tab to another group or to an edge to split,
// resize the splits, fold the side docks. The Output panel gets a line every second through
// `setProps`, which never undoes the layout. A file on the left opens as a closable panel; closing
// it sends `close` and the app removes the node. The bar at the bottom shows what the host reports.
// Run: pnpm dev dock   (after `pnpm bootstrap`)

import { createApp } from "gpui-bridge";

const app = await createApp({ debug: process.argv.includes("--debug") });

const status = app.text({ class: "text-sm text-muted-foreground", content: "Drag a tab to rearrange the panels." });
const report = (line: string) => {
  console.log(line);
  status.set({ content: line });
  app.commit();
};

const dock = app.dock({ class: "flex-1 border rounded-md overflow-hidden" });

// One closable panel per open file; closing it removes the node.
const open = new Map<string, ReturnType<typeof app.dockPanel>>();
function openFile(name: string) {
  if (open.has(name)) return;
  const panel = app
    .dockPanel({ title: name, closable: true })
    .on("close", () => {
      panel.remove();
      open.delete(name);
      report(`close ${name}`);
    })
    .append(app.text({ class: "p-3 font-mono text-sm", content: `// ${name}\nexport {};` }));
  open.set(name, panel);
  dock.append(panel);
  report(`open ${name}`);
}

const files = app.div({ class: "flex flex-col gap-1 p-2" }).append(
  ...["main.ts", "app.ts", "styles.css"].map((name) =>
    app.button({ label: name, variant: "ghost", class: "justify-start" }).on("click", () => openFile(name)),
  ),
);

const log = app.text({ class: "p-3 font-mono text-sm", content: "" });
let lines: string[] = [];
let ticks = 0;
const timer = setInterval(() => {
  ticks++;
  lines = [...lines, `${new Date().toLocaleTimeString()}  tick ${ticks}`].slice(-6);
  log.set({ content: lines.join("\n") });
  app.commit();
}, 1000);
// Closing the window ends the host; a commit after that would throw.
app.on("exit", () => clearInterval(timer));

dock.append(
  app.dockPanel({ title: "Files", placement: "left" }).append(files),
  app.dockPanel({ title: "Notes" }).append(
    app.div({ class: "p-3 flex flex-col gap-2" }).append(
      app.text({ content: "What you type here stays when the panel moves." }),
      app.input({ placeholder: "A note" }),
    ),
  ),
  app.dockPanel({ title: "Output", placement: "bottom" }).append(log),
);
openFile("main.ts");

app.window({
  title: "Dock",
  width: 960,
  height: 640,
  root: app.div({ class: "size-full flex flex-col gap-3 p-4" }).append(dock, status),
});
app.commit();
