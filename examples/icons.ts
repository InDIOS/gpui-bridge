// Icons of the app's own: SVGs registered with `icons` (PROTOCOL.md section 29) and used like the
// bundled ones, in an Icon and in `icon` props. "Swap" registers `rocket` again with another
// drawing, and every place that shows it changes.
// Run: pnpm dev icons   (after `pnpm bootstrap`)

import { createApp } from "gpui-bridge";

const app = await createApp({ debug: process.argv.includes("--debug") });

// Stroked 24×24 drawings; `currentColor` makes them take the text color, like the bundled icons.
const svg = (body: string) =>
  `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">${body}</svg>`;
const rocket = svg('<path d="M12 2c3 2 5 6 5 10l-2 4H9l-2-4c0-4 2-8 5-10z"/><circle cx="12" cy="9" r="2"/><path d="M9 16l-2 5 3-2M15 16l2 5-3-2"/>');
const planet = svg('<circle cx="12" cy="12" r="6"/><path d="M3 15c2 2 16-4 18-8"/>');
const leaf = svg('<path d="M5 19c0-9 6-14 15-15-1 9-6 15-15 15z"/><path d="M5 19l8-8"/>');

app.icons({ rocket, planet, leaf });

const root = app.div({ class: "flex flex-col size-full gap-4 p-6 bg-background" });

// The same name at several sizes and colors.
root.append(
  app.div({ class: "flex items-end gap-4" }).append(
    app.icon({ name: "rocket", class: "size-4 text-foreground" }),
    app.icon({ name: "rocket", class: "size-6 text-primary" }),
    app.icon({ name: "planet", class: "size-8 text-muted-foreground" }),
    app.icon({ name: "leaf", class: "size-12 text-success" }),
  ),
);

// In the `icon` props of other components, next to a bundled icon.
root.append(
  app.div({ class: "flex items-center gap-2" }).append(
    app.tag({ label: "Launch", icon: "rocket", variant: "primary" }),
    app.tag({ label: "Garden", icon: "leaf" }),
    app.tag({ label: "Bundled", icon: "check" }),
    app.toggle({ label: "Orbit", icon: "planet", variant: "outline" }),
  ),
);

let swapped = false;
const swap = app.button({ label: "Swap", icon: "refresh-cw", variant: "primary" }).on("click", () => {
  swapped = !swapped;
  app.icons({ rocket: swapped ? planet : rocket }).commit();
});
// Buttons: icon + label, icon only (the tooltip names it), and the same with a bundled icon.
root.append(
  app.div({ class: "flex items-center gap-2" }).append(
    swap,
    app.button({ label: "Launch", icon: "rocket", variant: "secondary" }),
    app.button({ icon: "leaf", tooltip: "Garden", variant: "ghost" }),
    app.button({ icon: "search", tooltip: "Search", variant: "ghost" }),
    app.button({ label: "Delete", icon: "delete", variant: "danger" }),
  ),
);
root.append(app.empty({ icon: "rocket", title: "Nothing launched yet", description: "Every rocket here is the app's own SVG." }));

app.window({ title: "Icons", width: 520, height: 420, root });
app.commit();
