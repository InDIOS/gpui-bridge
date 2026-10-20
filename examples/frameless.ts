// Frameless window: no system title bar; the app draws its own with `windowControl` areas,
// which Windows handles natively (drag, snap, double-click to maximize, the buttons).
// Run: node examples/frameless.ts   (after `npm run build` in client-node/ and `cargo build` in host/)

import { createApp } from "gpui-bridge";

const app = await createApp({ debug: process.argv.includes("--debug") });

const root = app.div({ class: "flex flex-col size-full bg-background text-foreground border border-border" });

const titleBar = app.div({ class: "flex flex-row items-center h-9 shrink-0 bg-muted border-b border-border" });
const drag = app.div({ class: "flex flex-row flex-1 items-center gap-2 h-full px-3", windowControl: "drag" });
drag.append(
  app.icon({ name: "palette", class: "size-4 text-primary" }),
  app.text({ content: "Frameless", class: "text-sm font-medium" }),
);
const controls = app.div({ class: "flex flex-row h-full" });
for (const [control, icon, hover] of [
  ["minimize", "window-minimize", "hover:bg-accent"],
  ["maximize", "window-maximize", "hover:bg-accent"],
  ["close", "window-close", "hover:bg-danger"],
] as const) {
  const button = app.div({ class: `flex items-center justify-center w-12 h-full ${hover}`, windowControl: control });
  button.append(app.icon({ name: icon, class: "size-4" }));
  controls.append(button);
}
titleBar.append(drag, controls);

const body = app.div({ class: "flex flex-col flex-1 items-center justify-center gap-3" });
let clicks = 0;
const label = app.text({ content: "Drag the bar, double-click it, or use the buttons.", class: "text-sm text-muted-foreground" });
const button = app.button({ label: "A normal button: 0", variant: "primary" });
button.on("click", () => {
  button.set({ label: `A normal button: ${++clicks}` });
  app.commit();
});
body.append(label, button);

root.append(titleBar, body);
app.on("windowClosed", () => console.log("windowClosed"));
app.window({ title: "Frameless", width: 480, height: 300, frameless: true, root });
app.commit();
