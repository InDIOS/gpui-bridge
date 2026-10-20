// Transport and tree-apply bench. Talks to `host --no-window` directly, so it measures the
// channel and the host with the client library's encoding out of the way.
//
//   node bench/tree.mjs [1000 10000 50000]      (default: 0 1000 10000 50000)
//
// For each size it creates N divs under a clickable root, commits, and measures the round trip
// of a `simulate` click. The same round trip is measured right after, with the host idle: the
// difference is the cost of parsing and applying N nodes. Nothing is rendered (`--no-window`),
// so this says nothing about drawing; `bench/render.mjs` covers that.

import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const exe = process.platform === "win32" ? "gpui-bridge-host.exe" : "gpui-bridge-host";
const host = process.env.GPUI_BRIDGE_HOST ?? join(here, "..", "host", "target", "debug", exe);
if (!existsSync(host)) {
  console.error(`no host at ${host}; build it (cargo build in host/) or set GPUI_BRIDGE_HOST`);
  process.exit(1);
}

const sizes = process.argv.slice(2).map(Number).filter((n) => Number.isInteger(n) && n >= 0);
const RUNS = 3;
const median = (xs) => xs.slice().sort((a, b) => a - b)[Math.floor(xs.length / 2)];
const round = (ms) => Number(ms.toFixed(1));

/** A host in `--no-window`, with its stdout read by line. */
class Session {
  constructor() {
    this.child = spawn(host, ["--no-window"], { stdio: ["pipe", "pipe", "ignore"] });
    this.pending = [];
    this.lines = [];
    let buffer = "";
    this.child.stdout.on("data", (chunk) => {
      buffer += chunk;
      let end;
      while ((end = buffer.indexOf("\n")) >= 0) {
        const line = buffer.slice(0, end);
        buffer = buffer.slice(end + 1);
        const resolve = this.pending.shift();
        if (resolve) resolve(line);
        else this.lines.push(line);
      }
    });
  }

  send(line) {
    this.child.stdin.write(line + "\n");
  }

  /** Writes a line and resolves when the pipe has taken it. */
  flush(line) {
    return new Promise((resolve) => this.child.stdin.write(line + "\n", resolve));
  }

  next() {
    const line = this.lines.shift();
    if (line !== undefined) return Promise.resolve(line);
    return new Promise((resolve) => this.pending.push(resolve));
  }

  async close() {
    this.child.stdin.end();
    await new Promise((resolve) => this.child.once("close", resolve));
  }
}

/** A click on the root: its event comes back only after the commit before it was applied. */
async function clickRoundTrip(session) {
  const start = performance.now();
  session.send(JSON.stringify({ op: "simulate", id: 1, event: "click" }));
  await session.next();
  return performance.now() - start;
}

async function run(n) {
  const session = new Session();
  session.send(JSON.stringify({ op: "hello", protocol: 1 }));
  await session.next();

  const ops = [{ op: "create", id: 1, type: "div", events: ["click"] }];
  for (let i = 0; i < n; i++) {
    const id = i + 2;
    ops.push({ op: "create", id, type: "div" }, { op: "append", parent: 1, child: id });
  }

  const encode = performance.now();
  const messages = ops.map((op) => JSON.stringify(op));
  const commit = JSON.stringify({ op: "commit" });
  const encodeMs = performance.now() - encode;
  const bytes = messages.reduce((sum, message) => sum + message.length + 1, commit.length + 1);

  const write = performance.now();
  for (const message of messages) session.flush(message);
  await session.flush(commit);
  const writeMs = performance.now() - write;

  const withApply = await clickRoundTrip(session);
  const idle = [];
  for (let i = 0; i < RUNS; i++) idle.push(await clickRoundTrip(session));
  const idleMs = median(idle);

  await session.close();
  return { n, ops: messages.length + 1, bytes, encodeMs, writeMs, withApply, idleMs, applyMs: withApply - idleMs };
}

const wanted = sizes.length ? sizes : [0, 1000, 10000, 50000];
console.log(`host ${host}`);
console.log("| nodes | ops | bytes | stringify ms | flush ms | click ms | idle click ms | apply ms |");
console.log("|---|---|---|---|---|---|---|---|");
for (const n of wanted) {
  const r = await run(n);
  console.log(
    `| ${r.n} | ${r.ops} | ${r.bytes} | ${round(r.encodeMs)} | ${round(r.writeMs)} | ${round(r.withApply)} | ${round(r.idleMs)} | ${round(r.applyMs)} |`,
  );
}