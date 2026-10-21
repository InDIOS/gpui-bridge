// One entry point for the examples, so a new checkout only needs
// `pnpm bootstrap` and then, say, `pnpm dev counter`.
//
//   pnpm dev                      # examples/counter.ts
//   pnpm dev todo                 # examples/todo.ts
//   pnpm dev counter.py           # examples/counter.py
//   pnpm dev react/todo           # examples/react/todo.tsx (compiled first)
//   pnpm dev forms --debug        # extra arguments go to the example (pnpm swallows `--`)

import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const exe = process.platform === "win32" ? "gpui-bridge-host.exe" : "gpui-bridge-host";

// The same order the clients use: the debug build is the one `pnpm bootstrap` and every
// `cargo build` refresh, so it is what an example must run while developing. A release build
// (packaging, or a checkout without a debug one) is the fallback: preferring it runs a host
// older than the working tree.
function hostPath() {
  for (const profile of ["debug", "release"]) {
    const candidate = join(root, "target", profile, exe);
    if (existsSync(candidate)) return candidate;
  }
  console.error("host not built: run `pnpm bootstrap` (or `pnpm host:build`).");
  process.exit(1);
}

const separator = process.argv.indexOf("--");
const args = process.argv.slice(2, separator === -1 ? undefined : separator);
// Everything after the example name goes to it. pnpm strips the `--` itself, so both
// `pnpm dev forms --debug` and `pnpm dev forms -- --debug` reach the example.
const extra = separator === -1 ? args.slice(1) : process.argv.slice(separator + 1);
const name = args[0] ?? "counter";

const host = hostPath();
const env = { ...process.env, GPUI_BRIDGE_HOST: host };
// Which host a manual test is running: a stale binary looks exactly like a broken feature.
if (extra.includes("--debug")) console.log(`gpui-bridge host: ${host}`);

function run(command, commandArgs) {
  const result = spawnSync(command, commandArgs, { cwd: root, env, stdio: "inherit", shell: process.platform === "win32" });
  process.exit(result.status ?? 1);
}

const isTsx = name.endsWith(".tsx") || existsSync(join(root, "examples", `${name}.tsx`));

if (name.endsWith(".py")) {
  run("python", [join("examples", name), ...extra]);
} else if (isTsx) {
  run("pnpm", ["--filter", "gpui-bridge-examples", "build:react"]);
  run("node", [join("examples", "dist", name.replace(/\.tsx$/, ".js")), ...extra]);
} else {
  const file = /\.(ts|js)$/.test(name) ? name : `${name}.ts`;
  run("node", [join("examples", file), ...extra]);
}
