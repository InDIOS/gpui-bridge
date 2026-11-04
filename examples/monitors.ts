// Monitors (M19): arrange screens on a canvas, as a display settings page does. Fake data, no
// screen API. Drag a screen (the host moves it and snaps it to the others' edges), click it to
// select it, turn it off, and drag a "Turned off" chip back onto the canvas to turn it on there.
// The app owns the layout: it accepts each `dragEnd` by writing the position back, and puts a
// screen back where it was if it was left on top of another one.
// Run: pnpm dev monitors   (after `pnpm bootstrap`)

import { createApp, type UINode } from "gpui-bridge";

const app = await createApp({ debug: process.argv.includes("--debug") });

interface Monitor {
  id: string;
  label: string;
  width: number;
  height: number;
  x: number;
  y: number;
  on: boolean;
}

// Tiles are the resolution at 1/12 scale.
const monitors: Monitor[] = [
  { id: "1", label: "Built-in display", width: 1920, height: 1080, x: 40, y: 120, on: true },
  { id: "2", label: "DELL U2723QE", width: 2560, height: 1440, x: 200, y: 80, on: true },
  { id: "3", label: "LG 24MP400", width: 1920, height: 1080, x: 413, y: 120, on: true },
  { id: "4", label: "Projector", width: 1280, height: 1024, x: 0, y: 0, on: false },
];
const SCALE = 12;
const CANVAS = { width: 640, height: 340 };
const size = (m: Monitor) => ({ w: Math.round(m.width / SCALE), h: Math.round(m.height / SCALE) });

let selected = "1";
const tiles = new Map<string, UINode<"div">>();
const chips = new Map<string, UINode<"div">>();

const root = app.div({ class: "flex flex-col gap-4 p-6 size-full bg-background" });
const title = app.text({ content: "Display layout", class: "text-2xl font-bold text-foreground" });
const hint = app.text({
  content: "Drag the screens to arrange them. Click one to select it.",
  class: "text-sm text-muted-foreground",
});
// The canvas border is 1 px, so its inside is 2 px smaller than its classes.
const canvas = app.div({
  class: `relative w-[${CANVAS.width + 2}px] h-[${CANVAS.height + 2}px] rounded-lg border border-border bg-muted bg-dots drag-over:border-primary drag-over:bg-accent`,
  dropTarget: true,
});
const offRow = app.div({ class: "flex flex-row gap-2 items-center" });
const details = app.text({ class: "text-sm text-foreground" });
const turnOff = app.button({ label: "Turn off", variant: "secondary" });

function tileClass(m: Monitor): string {
  const { w, h } = size(m);
  const look =
    m.id === selected
      ? "bg-gradient-to-br from-info to-primary shadow-xl shadow-info border border-info"
      : "bg-gradient-to-br from-primary to-info shadow-lg";
  return `absolute left-[${m.x}px] top-[${m.y}px] w-[${w}px] h-[${h}px] rounded-lg flex flex-col items-center justify-center cursor-pointer ${look}`;
}

function overlaps(a: Monitor): boolean {
  const sa = size(a);
  return monitors.some((b) => {
    if (b === a || !b.on) return false;
    const sb = size(b);
    return a.x < b.x + sb.w && b.x < a.x + sa.w && a.y < b.y + sb.h && b.y < a.y + sa.h;
  });
}

function showDetails() {
  const m = monitors.find((m) => m.id === selected && m.on);
  details.set({
    content: m ? `${m.id}: ${m.label} — ${m.width}×${m.height} at ${m.x}, ${m.y}` : "No screen selected.",
  });
  turnOff.set({ disabled: !m || monitors.filter((m) => m.on).length === 1 });
}

function select(id: string) {
  const before = selected;
  selected = id;
  for (const key of [before, id]) {
    const m = monitors.find((m) => m.id === key);
    if (m?.on) tiles.get(key)?.set({ class: tileClass(m) });
  }
  showDetails();
}

function addTile(m: Monitor) {
  const tile = app.div({ class: tileClass(m), drag: true, magnetic: true, a11y: { label: `Screen ${m.id}, ${m.label}` } });
  tile.append(
    app.text({ content: m.id, class: "text-4xl font-bold text-primary-foreground" }),
    app.text({ content: `${m.width}×${m.height}`, class: "text-xs text-primary-foreground" }),
  );
  tile.on("click", () => {
    select(m.id);
    app.commit();
  });
  tile.on("dragEnd", ({ value }) => {
    const before = { x: m.x, y: m.y };
    Object.assign(m, value);
    // Left on top of another screen: back where it was (the class changes, so the host moves it).
    if (overlaps(m)) Object.assign(m, before);
    selected = m.id;
    for (const other of monitors) if (other.on) tiles.get(other.id)?.set({ class: tileClass(other) });
    showDetails();
    app.commit();
  });
  tiles.set(m.id, tile);
  canvas.append(tile);
}

function addChip(m: Monitor) {
  const chip = app.div({
    class: "flex flex-row gap-2 items-center px-3 py-1 rounded-full border border-border bg-secondary cursor-pointer",
    dragData: m.id,
  });
  chip.append(app.text({ content: `${m.id} · ${m.label}`, class: "text-sm text-secondary-foreground" }));
  chips.set(m.id, chip);
  offRow.append(chip);
}

turnOff.on("click", () => {
  const m = monitors.find((m) => m.id === selected && m.on);
  if (!m) return;
  m.on = false;
  tiles.get(m.id)?.remove();
  tiles.delete(m.id);
  addChip(m);
  selected = monitors.find((m) => m.on)?.id ?? "";
  if (selected) select(selected);
  else showDetails();
  app.commit();
});

// A chip left on the canvas turns that screen on where it was dropped, kept inside the canvas.
canvas.on("dropNode", ({ value }) => {
  const m = monitors.find((m) => m.id === value.data && !m.on);
  if (!m) return;
  const { w, h } = size(m);
  m.x = Math.min(value.x, CANVAS.width - w);
  m.y = Math.min(value.y, CANVAS.height - h);
  m.on = true;
  chips.get(m.id)?.remove();
  chips.delete(m.id);
  selected = m.id;
  addTile(m);
  for (const other of monitors) if (other.on) tiles.get(other.id)?.set({ class: tileClass(other) });
  showDetails();
  app.commit();
});

offRow.append(app.text({ content: "Turned off (drag onto the canvas):", class: "text-sm text-muted-foreground" }));
for (const m of monitors) (m.on ? addTile : addChip)(m);
showDetails();

root.append(title, hint, canvas, offRow, app.div({ class: "flex flex-row gap-3 items-center" }).append(details, turnOff));
app.window({ title: "Monitors", width: 720, height: 600, root });
app.commit();
