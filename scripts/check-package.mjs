// Installs the packed npm tarballs and the Python wheel in a folder *outside* this repository
// and drives a headless app with each client. That is the packaging acceptance test: no
// `GPUI_BRIDGE_HOST`, no Rust toolchain and nothing from the checkout but the artifacts, so the
// host that runs is the one the packages carry.
//
//   pnpm pack:node && pnpm --dir packages/host-<platform>-<arch> pack && pnpm wheel
//   node scripts/check-package.mjs            # or: pnpm check:package
//   node scripts/check-package.mjs --keep     # leave the folder behind to look at it

import { execFileSync } from "node:child_process";
import { existsSync, mkdtempSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const keep = process.argv.includes("--keep");

/** The npm CLI as a script, so it can be run without a shell: on Windows `npm` is a `.cmd`
 *  shim, and spawning one of those is refused (EINVAL). */
function npmCli() {
  const bin = dirname(process.execPath);
  const cli = [
    join(bin, "node_modules", "npm", "bin", "npm-cli.js"), // Windows
    join(bin, "..", "lib", "node_modules", "npm", "bin", "npm-cli.js"), // Linux and macOS
  ].find(existsSync);
  return cli === undefined ? ["npm"] : [process.execPath, cli];
}

const npm = npmCli();

/** The files in `dir` that end with `suffix` (empty if the folder does not exist). */
function files(dir, suffix) {
  return existsSync(dir)
    ? readdirSync(dir)
        .filter((name) => name.endsWith(suffix))
        .map((name) => join(dir, name))
    : [];
}

const packs = [
  ...files(join(root, "packages", `host-${process.platform}-${process.arch}`), ".tgz"),
  ...files(join(root, "packages", "client-node"), ".tgz"),
];
const wheels = files(join(root, "packages", "client-python", "dist"), ".whl");
if (packs.length < 2 || wheels.length === 0) {
  console.error(
    "pack first: `pnpm pack:node`, `pnpm --dir packages/host-<platform>-<arch> pack` and `pnpm wheel`.",
  );
  process.exit(1);
}

const work = mkdtempSync(join(tmpdir(), "gpui-bridge-"));
console.log(`outside the repository: ${work}`);

function run(command, args) {
  execFileSync(command, args, { cwd: work, stdio: ["ignore", "inherit", "inherit"] });
}

try {
  writeFileSync(join(work, "package.json"), JSON.stringify({ private: true, type: "module" }, null, 2) + "\n");
  console.log(`installing ${packs.length} npm tarballs`);
  run(npm[0], [...npm.slice(1), "install", "--no-audit", "--no-fund", ...packs]);

  writeFileSync(
    join(work, "app.mjs"),
    `import { createApp, defaultHostPath } from "gpui-bridge";

const app = await createApp({ headless: true });
console.log("node resolved:", defaultHostPath());
const label = app.text({ content: "packaged" });
app.window({ root: app.div().append(label) });
app.commit();
label.set({ content: "still packaged" });
app.commit();
const exit = await app.close();
if (exit.code !== 0) throw new Error(\`the host exited with \${exit.code}\`);
console.log("node: the packaged client drove the packaged host");
`,
  );
  run(process.execPath, ["app.mjs"]);

  run("python", ["-m", "venv", "venv"]);
  const python = join(work, "venv", ...(process.platform === "win32" ? ["Scripts", "python.exe"] : ["bin", "python"]));
  run(python, ["-m", "pip", "install", "--quiet", ...wheels]);
  writeFileSync(
    join(work, "app.py"),
    `import gpui_bridge
from gpui_bridge import App

print("python resolved:", gpui_bridge.default_host_path())
app = App(headless=True)
label = app.text(content="packaged")
app.window(root=app.div().append(label))
app.commit()
label.set(content="still packaged")
app.commit()
code = app.close().code
if code != 0:
    raise SystemExit(f"the host exited with {code}")
print("python: the packaged client drove the packaged host")
`,
  );
  run(python, ["app.py"]);
  console.log("both clients ran the host that came in the packages");
} finally {
  if (keep) console.log(`kept ${work}`);
  else rmSync(work, { recursive: true, force: true });
}
