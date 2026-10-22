// Stages the *already built* host binary into the packages that ship it as an asset (the npm
// platform package and the Python wheel), and writes `host.json` next to it so a bug report
// can say which host a client is running.
//
//   pnpm host:release                                # cargo build -p gpui-bridge-host --release
//   node scripts/stage-host.mjs                      # -> npm platform package + wheel
//   node scripts/stage-host.mjs --to python          # only the wheel
//   node scripts/stage-host.mjs --tag win32-x64      # the platform the asset is for
//   node scripts/stage-host.mjs --triple x86_64-pc-windows-msvc
//
// It never compiles: a release build takes minutes, and doing it once here (or in CI,
// where the binary travels as an artifact) is the point of the staging step.

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const native = `${process.platform}-${process.arch}`;

function argument(name, fallback) {
  const index = process.argv.indexOf(`--${name}`);
  return index === -1 ? fallback : process.argv[index + 1];
}

/** `x86_64-pc-windows-msvc` -> `win32-x64`: the tag the clients build from `process.platform`
 *  and `process.arch`, and the suffix of the npm platform package (`gpui-bridge-win32-x64`). */
function tagOf(triple) {
  const os = ["windows", "darwin", "linux"].find((name) => triple.includes(name));
  if (os === undefined) return undefined;
  return `${os === "windows" ? "win32" : os}-${/^(aarch64|arm64)/.test(triple) ? "arm64" : "x64"}`;
}

const targets = argument("to", "node,python").split(",").map((value) => value.trim());
const triple = argument("triple");
const tag = argument("tag", triple === undefined ? native : tagOf(triple));
if (tag === undefined) {
  console.error(`cannot tell which platform ${triple} is for; pass --tag <platform>-<arch>.`);
  process.exit(1);
}
// The tag, not the machine, decides: a cross target stages the binary Windows or Linux needs.
const exe = tag.startsWith("win32") ? "gpui-bridge-host.exe" : "gpui-bridge-host";
// One npm package per platform, so an install downloads a single binary.
const npmPackage = join(root, "packages", `host-${tag}`);
if (targets.includes("node") && !existsSync(join(npmPackage, "package.json"))) {
  console.error(`there is no npm package for ${tag}: expected ${join(npmPackage, "package.json")}.`);
  process.exit(1);
}

// `cargo build --target <triple>` puts the binary in target/<triple>/release; a native build
// (no `--triple`, or the host's own triple) puts it in target/release.
function defaultFrom() {
  const candidates = [
    ...(triple === undefined ? [] : [join(root, "target", triple, "release", exe)]),
    join(root, "target", "release", exe),
  ];
  return candidates.find(existsSync) ?? candidates[0];
}

const from = argument("from", defaultFrom());

if (!existsSync(from)) {
  console.error(`host not found at ${from}\nrun \`pnpm host:release\` first (or pass --from).`);
  process.exit(1);
}

// `cargo metadata` is authoritative, but this script also runs where only the artifact
// exists (CI packaging jobs), so fall back to the workspace Cargo.toml.
function hostVersion() {
  try {
    const metadata = JSON.parse(
      execFileSync(
        "cargo",
        ["metadata", "--no-deps", "--format-version", "1", "--manifest-path", join(root, "packages", "host", "Cargo.toml")],
        { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] },
      ),
    );
    return metadata.packages.find((entry) => entry.name === "gpui-bridge-host").version;
  } catch {
    const cargo = readFileSync(join(root, "Cargo.toml"), "utf8");
    return cargo.match(/\[workspace\.package\][\s\S]*?version = "([^"]+)"/)[1];
  }
}

const version = hostVersion();
const protocol = Number(
  readFileSync(join(root, "packages", "host", "src", "protocol.rs"), "utf8").match(/PROTOCOL_VERSION: u32 = (\d+)/)[1],
);

const bytes = readFileSync(from);
const hostJson =
  JSON.stringify(
    { version, protocol, target: tag, sha256: createHash("sha256").update(bytes).digest("hex") },
    null,
    2,
  ) + "\n";

const destinations = {
  node: join(npmPackage, "bin"),
  python: join(root, "packages", "client-python", "gpui_bridge", "bin"),
};

for (const target of targets) {
  const destination = destinations[target];
  if (!destination) {
    console.error(`unknown target ${target}; use node or python.`);
    process.exit(1);
  }
  mkdirSync(destination, { recursive: true });
  copyFileSync(from, join(destination, exe));
  writeFileSync(join(destination, "host.json"), hostJson);
  console.log(`host ${version} (protocol ${protocol}, ${tag}) -> ${destination}`);
}
