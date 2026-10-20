// Render bench: a window with N nodes, either quiet (`idle`) or with one spinning icon, which
// makes the host rebuild the whole window every frame (`spin`). This script only builds the tree
// and stays alive; bench/run.ps1 samples the host's CPU while it runs.
//
//   node bench/render.mjs <nodes> <idle|spin>

import { createApp } from "../client-node/dist/index.js";

const nodes = Number(process.argv[2] ?? 20000);
const mode = process.argv[3] ?? "idle";

const app = await createApp({ debug: process.argv.includes("--debug") });
const root = app.div({ class: "flex flex-col p-2" });
for (let i = 0; i < nodes; i++) root.append(app.text({ content: `row ${i}`, class: "text-sm" }));
if (mode === "spin") root.append(app.icon({ name: "loader-circle", class: "size-4 animate-spin" }));
app.window({ title: `bench ${nodes} ${mode}`, width: 900, height: 700, root });
app.commit();

// Let the host finish the first build before the driver starts sampling. Two sample windows
// follow (see bench/run.ps1), so a slow first paint does not get read as steady state.
await new Promise((resolve) => setTimeout(resolve, 5000));
console.log(`READY nodes=${nodes} mode=${mode}`);

await new Promise((resolve) => setTimeout(resolve, 25000));
await app.close();
process.exit(0);