// System tray: closing the window keeps the app in the tray; clicking the icon or "Show"
// opens it again. The tray menu has a checked item the app toggles and a submenu.
// Run: node examples/tray.ts   (after `npm run build` in client-node/ and `cargo build` in host/)

import { fileURLToPath } from "node:url";
import { createApp, type MenuItem } from "gpui-bridge";

const app = await createApp({ debug: process.argv.includes("--debug") });
const icon = fileURLToPath(new URL("tray-icon.png", import.meta.url));

let notify = true;
let shown = false;
let clicks = 0;
const menu = (): MenuItem[] => [
  { id: "show", label: "Show window" },
  { id: "notify", label: "Notifications", checked: notify },
  { label: "Status", items: [{ id: "status:online", label: "Online" }, { id: "status:away", label: "Away" }] },
  { id: "disabled", label: "Not available", disabled: true },
  { separator: true },
  { id: "quit", label: "Quit" },
];

const root = app.div({ class: "flex flex-col size-full items-center justify-center gap-3 bg-background text-foreground" });
const status = app.text({ content: "Close this window: the app stays in the tray.", class: "text-sm text-muted-foreground" });
const button = app.button({ label: "Clicks: 0", variant: "primary" });
button.on("click", () => {
  button.set({ label: `Clicks: ${++clicks}` });
  app.commit();
});
root.append(app.text({ content: "Tray example", class: "text-lg font-semibold" }), status, button);

function show() {
  if (shown) return;
  shown = true;
  app.window({ title: "Tray example", width: 420, height: 260, root });
}

app.on("windowClosed", () => {
  shown = false;
  status.set({ content: "Back from the tray." });
  console.log("windowClosed (still in the tray)");
});
app.on("tray", (event) => {
  console.log("tray", event);
  if (event.event === "click") return void (show(), app.commit());
  switch (event.item) {
    case "show":
      show();
      break;
    case "notify":
      notify = !notify;
      app.tray({ menu: menu() });
      break;
    case "quit":
      // Without a window, removing the tray ends the host; with one open, close() does.
      app.removeTray().commit();
      if (shown) void app.close();
      return;
    default:
      status.set({ content: `Tray menu: ${event.item}` });
  }
  app.commit();
});

app.tray({ icon, tooltip: "gpui-bridge tray example", menu: menu() });
show();
app.commit();
