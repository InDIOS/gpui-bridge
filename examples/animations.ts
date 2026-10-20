// Animations: a spinner (`animate-spin`), a loading skeleton (`animate-pulse`), rows whose
// `hover:` color fades in (`transition`), and a box whose color and opacity change smoothly.
// Run: node examples/animations.ts   (after `npm run build` in client-node/ and `cargo build` in host/)

import { createApp } from "gpui-bridge";

const app = await createApp({ debug: process.argv.includes("--debug") });
let dark = false;

const root = app.div({ class: "flex flex-col size-full gap-4 p-6 bg-background text-foreground transition duration-500" });

const loading = app.div({ class: "flex flex-row items-center gap-3" });
loading.append(
  app.icon({ name: "loader-circle", class: "size-6 text-primary animate-spin" }),
  app.text({ content: "Loading…", class: "text-sm" }),
);

const skeleton = app.div({ class: "flex flex-col gap-2 animate-pulse" });
skeleton.append(app.div({ class: "h-4 w-64 rounded-md bg-muted" }), app.div({ class: "h-4 w-48 rounded-md bg-muted" }));

const rows = app.div({ class: "flex flex-col w-80 border border-border rounded-lg" });
for (const [label, timing] of [["Slow hover (1 s)", "duration-1000"], ["Default hover (150 ms)", ""], ["Linear hover (500 ms)", "duration-500 ease-linear"]]) {
  const row = app.div({ class: `flex px-3 py-2 rounded-lg cursor-pointer transition ${timing} hover:bg-accent hover:text-primary` });
  row.append(app.text({ content: label, class: "text-sm" }));
  rows.append(row);
}

let on = false;
const box = app.div({ class: "size-24 rounded-lg transition duration-700 bg-primary opacity-100" });
const toggle = app.button({ label: "Toggle box", variant: "primary" }).on("click", () => {
  on = !on;
  box.set({ class: `size-24 rounded-lg transition duration-700 ${on ? "bg-danger opacity-30" : "bg-primary opacity-100"}` });
  app.commit();
});
const theme = app.button({ label: "Dark mode" }).on("click", () => {
  dark = !dark;
  theme.set({ label: dark ? "Light mode" : "Dark mode" });
  app.window({ theme: dark ? "dark" : "light" });
  app.commit();
});
const controls = app.div({ class: "flex flex-row items-center gap-3" });
controls.append(toggle, theme, box);

root.append(loading, skeleton, rows, controls);
app.window({ title: "Animations", width: 560, height: 460, root });
app.commit();
