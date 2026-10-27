// Canvas: the client draws with rect, line and path; each press adds a square where it happened.
// Run: node examples/canvas.ts [--stress 5000]   (after `npm run build` in client-node/ and `cargo build` in host/)

import { createApp, type CanvasCommand } from "gpui-bridge";

const app = await createApp({ debug: process.argv.includes("--debug") });

const commands: CanvasCommand[] = [
  { op: "rect", x: 20, y: 20, w: 160, h: 90, color: "primary", corner_radius: 8 },
  { op: "line", x1: 20, y1: 140, x2: 360, y2: 140, color: "danger", width: 2 },
  { op: "path", d: "M220 20 L360 20 L290 110 Z", color: "chart-2", width: 3 },
];
// `--stress 5000` adds that many small squares, to feel the cost of repainting a long list on each press.
const stress = process.argv.indexOf("--stress");
const extra = stress < 0 ? 0 : Number(process.argv[stress + 1] ?? 5000);
for (let i = 0; i < extra; i++) commands.push({ op: "rect", x: (i * 7) % 380, y: 150 + ((i * 13) % 100), w: 4, h: 4, color: "muted" });

const root = app.div({ class: "flex flex-col gap-2 p-4 size-full bg-background" });
const label = app.text({ content: "Click the canvas", class: "text-sm text-muted-foreground" });
const canvas = app.canvas({ class: "h-64 w-full border border-border", commands });

canvas.on("pointerDown", ({ value: { x, y } }) => {
  commands.push({ op: "rect", x: x - 5, y: y - 5, w: 10, h: 10, color: "success" });
  canvas.set({ commands });
  label.set({ content: `x ${x.toFixed(1)}, y ${y.toFixed(1)} · ${commands.length} commands` });
  app.commit();
});

root.append(label, canvas);
app.window({ title: "Canvas", width: 420, height: 360, root });
app.commit();
