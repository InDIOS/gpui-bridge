// Render bench: a window with N nodes, either quiet (`idle`), with one spinning icon, which
// makes the host rebuild the whole window every frame (`spin`), or with the client changing one
// row and committing 60 times a second, like live data (`update`). This script only builds the
// tree and stays alive; benchmarks/run.ps1 samples the host's CPU while it runs.
//
//   node benchmarks/render.mjs <nodes> <idle|spin|update>

import { createApp } from "../packages/client-node/dist/index.js";

const nodes = Number(process.argv[2] ?? 20000);
const mode = process.argv[3] ?? "idle";

const app = await createApp({ debug: process.argv.includes("--debug") });
const root = app.div({ class: "flex flex-col p-2" });
const rows = [];
for (let i = 0; i < nodes; i++) rows.push(app.text({ content: `row ${i}`, class: "text-sm" }));
for (const row of rows) root.append(row);
if (mode === "spin") root.append(app.icon({ name: "loader-circle", class: "size-4 animate-spin" }));
app.window({ title: `bench ${nodes} ${mode}`, width: 900, height: 700, root });
app.commit();

// Let the host finish the first build before the driver starts sampling. Two sample windows
// follow (see benchmarks/run.ps1), so a slow first paint does not get read as steady state.
await new Promise((resolve) => setTimeout(resolve, 5000));
if (mode === "update") {
  let tick = 0;
  setInterval(() => {
    rows[0].set({ content: `tick ${tick++}` });
    app.commit();
  }, 16);
  // One second of updates before READY, so the sample starts in steady state.
  await new Promise((resolve) => setTimeout(resolve, 1000));
}
console.log(`READY nodes=${nodes} mode=${mode}`);

await new Promise((resolve) => setTimeout(resolve, 25000));
await app.close();
process.exit(0);