// Counter: a text and two buttons, driven entirely from Node.
// Run: node examples/counter.ts   (after `npm run build` in client-node/ and `cargo build` in host/)

import { createApp } from "gpui-bridge";

const app = await createApp({ debug: process.argv.includes("--debug") });

let count = 0;

const root = app.div({ class: "flex flex-col items-center justify-center gap-4 p-6 size-full bg-background" });
const label = app.text({ content: "0", class: "text-xl font-bold text-foreground" });
const buttons = app.div({ class: "flex flex-row gap-2" });
const subtract = app.button({ label: "Subtract", variant: "secondary" });
const add = app.button({ label: "Add", variant: "primary" });

function update(delta: number) {
  count += delta;
  label.set({ content: String(count) });
  app.commit();
}

subtract.on("click", () => update(-1));
add.on("click", () => update(1));

buttons.append(subtract, add);
root.append(label, buttons);
app.window({ title: "Counter", width: 320, height: 200, root });
app.commit();
