// Counter on txiki.js instead of Node: same client API, a ~5 MB runtime.
// Run:     GPUI_BRIDGE_HOST=<path to gpui-bridge-host> tjs run examples/counter-txiki.js
// Compile: see the txiki.js section of HISTORIAL.md (bundle, then `tjs compile`).

import { createTxikiApp } from "../packages/client-node/dist/txiki.js";

const app = await createTxikiApp({ debug: tjs.args.includes("--debug") });

let count = 0;

const root = app.div({ class: "flex flex-col items-center justify-center gap-4 p-6 size-full bg-background" });
const label = app.text({ content: "0", class: "text-xl font-bold text-foreground" });
const buttons = app.div({ class: "flex flex-row gap-2" });
const subtract = app.button({ label: "Subtract", variant: "secondary" });
const add = app.button({ label: "Add", variant: "primary" });

function update(delta) {
  count += delta;
  label.set({ content: String(count) });
  app.commit();
}

subtract.on("click", () => update(-1));
add.on("click", () => update(1));

buttons.append(subtract, add);
root.append(label, buttons);
app.window({ title: "Counter (txiki.js)", width: 320, height: 200, root });
app.commit();
