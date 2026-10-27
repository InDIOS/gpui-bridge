// The JSON floor: what one protocol line costs on its own, with no host in the way. Isolates
// `JSON.stringify` and `JSON.parse` at the sizes an image would have, so the numbers from
// `benchmarks/payload.mjs` (which include the host, the pipe and a retained copy) can be read
// against something.
//
//   node benchmarks/payload-micro.mjs

import { randomBytes } from "node:crypto";

const sizes = [16 * 1024, 64 * 1024, 256 * 1024, 1024 * 1024, 4 * 1024 * 1024];
const ITERATIONS = 20;
const median = (xs) => xs.slice().sort((a, b) => a - b)[Math.floor(xs.length / 2)];
const time = (run) => {
  const start = performance.now();
  for (let i = 0; i < ITERATIONS; i++) run(i);
  return (performance.now() - start) / ITERATIONS;
};

console.log("| raw bytes | base64 chars | line bytes | stringify ms | parse ms | retained MB |");
console.log("|---|---|---|---|---|---|");
for (const raw of sizes) {
  const base64 = randomBytes(raw).toString("base64");
  // The heap of the value itself, measured with `node --expose-gc` (n/a without it).
  if (global.gc) global.gc();
  const before = process.memoryUsage().heapUsed;
  const message = { op: "setProps", id: 2, props: { value: base64 } };
  let line = "";
  const stringify = median(Array.from({ length: 5 }, () => time(() => (line = JSON.stringify(message)))));
  const parse = median(Array.from({ length: 5 }, () => time(() => JSON.parse(line))));
  if (global.gc) global.gc();
  const retained = global.gc ? `${((process.memoryUsage().heapUsed - before) / 1048576).toFixed(1)}` : "n/a";
  console.log(`| ${raw} | ${base64.length} | ${line.length} | ${stringify.toFixed(2)} | ${parse.toFixed(2)} | ${retained} |`);
}