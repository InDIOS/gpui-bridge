// Payload bench: sends one large value in a single protocol line, the way a base64 image (path
// `d`) would travel today, and stays alive so benchmarks/run.ps1 can sample the host's memory.
//
//   node benchmarks/payload.mjs <rawBytes> <baseline|retained|cleared|path|image> [imagePath]
//
//   baseline  a one-character value, to measure the host with no payload
//   retained  a base64 value of `rawBytes`, kept in the tree (the worst case for path `d`)
//   cleared   the same, then emptied right after, to separate transient from retained cost
//   path      the bytes written to a temp file and a short path sent instead (path `c`)
//   image     a real image file shown as an `Image` node: the decode cost, which is the part
//             the transport encoding does not change
//
// Known bias: `Input.value` is a string the node keeps *and lays out as text*, so `retained`
// measures text shaping more than transport. Read those rows together with
// `benchmarks/payload-micro.mjs` (the encoding floor) and with `image` (the decode cost).

import { randomBytes } from "node:crypto";
import { writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createApp } from "../packages/client-node/dist/index.js";

const raw = Number(process.argv[2] ?? 65536);
const mode = process.argv[3] ?? "baseline";
const imagePath = process.argv[4];
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

const app = await createApp();
const root = app.div({ class: "flex flex-col gap-3 p-4 size-full" });
const field = mode === "image" ? app.image({ src: imagePath, fit: "contain", class: "w-full h-96" }) : app.input({ placeholder: "payload" });
root.append(field, app.text({ content: `payload bench: ${mode} ${raw}`, class: "text-sm" }));
app.window({ title: `bench payload ${mode} ${raw}`, width: 700, height: 500, root });
app.commit();

if (mode === "image") {
  // GPUI loads the asset asynchronously: give the decode time to happen before the sample.
  await sleep(2000);
  console.log(`APPLIED mode=image file=${imagePath} encodeMs=0.0`);
} else if (mode === "path") {
  const file = join(tmpdir(), `gpui-bridge-bench-${raw}.bin`);
  const write = performance.now();
  writeFileSync(file, randomBytes(raw));
  const fileMs = performance.now() - write;
  const encode = performance.now();
  field.set({ value: file });
  app.commit();
  const encodeMs = performance.now() - encode;
  console.log(`APPLIED mode=path raw=${raw} value=${file.length} fileMs=${fileMs.toFixed(1)} encodeMs=${encodeMs.toFixed(1)}`);
} else if (mode === "baseline") {
  const encode = performance.now();
  field.set({ value: "x" });
  app.commit();
  console.log(`APPLIED mode=baseline raw=${raw} value=1 encodeMs=${(performance.now() - encode).toFixed(1)}`);
} else {
  const base64 = randomBytes(raw).toString("base64");
  const encode = performance.now();
  field.set({ value: base64 });
  app.commit();
  const encodeMs = performance.now() - encode;
  console.log(`APPLIED mode=${mode} raw=${raw} value=${base64.length} encodeMs=${encodeMs.toFixed(1)}`);
  if (mode === "cleared") {
    await sleep(2000);
    field.set({ value: "" });
    app.commit();
    console.log("CLEARED");
  }
}

await sleep(2500);
console.log("DONE");
await app.close();
process.exit(0);