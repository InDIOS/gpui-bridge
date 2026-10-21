// The data components of the second batch: a Sidebar and three charts (a line with a thousand
// points, bars and a pie), next to a deep tree. Nothing here is drawn per point by the client:
// each node is one prop list, and the host parses it once per change.
// Run: node examples/dashboard.ts   (after `npm run build` in client-node/ and `cargo build` in host/)

import { createApp, type ChartDatum, type SidebarSection, type TreeNode } from "gpui-bridge";

const app = await createApp({ debug: process.argv.includes("--debug") });

const status = app.text({ content: "Pick an entry", class: "text-sm text-muted-foreground" });
const say = (content: string) => {
  status.set({ content });
  app.commit();
};

const sections: SidebarSection[] = [
  {
    label: "General",
    items: [
      { id: "overview", label: "Overview", icon: "layout-dashboard" },
      { id: "inbox", label: "Inbox", icon: "inbox" },
      { id: "files", label: "Files", icon: "folder" },
    ],
  },
  {
    label: "Advanced",
    items: [
      {
        id: "settings",
        label: "Settings",
        icon: "settings",
        children: [
          { id: "theme", label: "Theme" },
          { id: "palette", label: "Palette", icon: "palette", disabled: true },
        ],
      },
      { id: "reports", label: "Reports", icon: "chart-pie" },
    ],
  },
];

const sidebar = app
  .sidebar({ items: sections, value: "overview", a11y: { label: "Sections" } })
  .on("change", ({ value }) => say(`Sidebar: ${value}`));

// A thousand points: one `data` prop, read once by the host.
const series = (seed: number): ChartDatum[] =>
  Array.from({ length: 1000 }, (_, i) => ({
    label: String(i + 1),
    value: Math.round(50 + 28 * Math.sin((i + seed) / 40) + 9 * Math.sin((i + seed) / 7)),
  }));

let seed = 0;
let collapsedNow = false;
const line = app.chart({
  chart: "line",
  name: "Request rate",
  data: series(seed),
  curve: "natural",
  yAxis: true,
  a11y: { label: "Request rate, 1000 points" },
  class: "h-56 w-full border border-border rounded-md",
});

const bars = app.chart({
  chart: "bar",
  name: "Deploys",
  data: ["Jan", "Feb", "Mar", "Apr", "May", "Jun"].map((label, i) => ({
    label,
    value: 12 + i * 7 - (i % 2) * 5,
  })),
  color: "chart-2",
  yAxis: true,
  class: "h-56 flex-1 border border-border rounded-md",
});

const pie = app.chart({
  chart: "pie",
  data: [
    { label: "Rust", value: 62 },
    { label: "TypeScript", value: 21 },
    { label: "Python", value: 11 },
    { label: "Other", value: 6 },
  ],
  innerRadius: 34,
  padAngle: 0.03,
  class: "h-56 flex-1 border border-border rounded-md",
});

// A deep tree: 10 levels of two children each (2047 items), the first three levels open.
const deep = (depth: number, id: string): TreeNode => ({
  id,
  label: id.slice(id.lastIndexOf("/") + 1),
  expanded: depth > 7,
  children: depth === 0 ? undefined : [deep(depth - 1, `${id}/a`), deep(depth - 1, `${id}/b`)],
});

const tree = app
  .tree({ items: [deep(10, "root")], class: "h-64 overflow-hidden border border-border rounded-md" })
  .on("change", ({ value }) => say(`Tree: ${value}`));

const collapse = app
  .button({ label: "Toggle sidebar", variant: "secondary" })
  .on("click", () => {
    const collapsed = !collapsedNow;
    collapsedNow = collapsed;
    sidebar.set({ collapsed });
    app.commit();
  });

const refresh = app
  .button({ label: "Refresh the series", variant: "secondary" })
  .on("click", () => {
    seed += 13;
    const started = performance.now();
    line.set({ data: series(seed) });
    app.commit();
    say(`1000 points sent in ${(performance.now() - started).toFixed(1)} ms`);
  });

const label = (content: string) => app.text({ content, class: "text-sm font-medium" });

const main = app
  .div({ class: "flex flex-col flex-1 min-w-0 gap-4 p-4 overflow-y-scroll" })
  .append(
    app
      .div({ class: "flex items-center justify-between gap-2" })
      .append(
        app.text({ content: "Dashboard", class: "text-xl font-bold" }),
        app.div({ class: "flex items-center gap-2" }).append(collapse, refresh),
      ),
    status,
    app.div({ class: "flex flex-col gap-2" }).append(label("Request rate (1000 points)"), line),
    app
      .div({ class: "flex gap-4" })
      .append(
        app.div({ class: "flex flex-col flex-1 gap-2" }).append(label("Deploys"), bars),
        app.div({ class: "flex flex-col flex-1 gap-2" }).append(label("Languages"), pie),
      ),
    app.div({ class: "flex flex-col gap-2" }).append(label("A tree of 10 levels"), tree),
  );

const root = app.div({ class: "flex size-full bg-background text-foreground" }).append(sidebar, main);
app.window({ title: "Sidebar, charts and a deep tree", width: 1100, height: 900, root });
app.commit();
