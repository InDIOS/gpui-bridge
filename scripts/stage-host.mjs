// Stages the *already built* host binary into the client packages that ship it as an
// asset (npm tarball and Python wheel), and writes `host.json` next to it so a bug
// report can say which host a client is running.
//
//   pnpm host:release                                # cargo build -p gpui-bridge-host --release
//   node scripts/stage-host.mjs                      # -> node + python assets
//   node scripts/stage-host.mjs --to python          # only the wheel
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
const exe = process.platform === "win32" ? "gpui-bridge-host.exe" : "gpui-bridge-host";

function argument(name, fallback) {
  const index = process.argv.indexOf(`--${name}`);
  return index === -1 ? fallback : process.argv[index + 1];
}

const targets = (argument("to", "node,python")).split(",").map((value) => value.trim());
const triple = argument("triple", `${process.platform}-${process.arch}`);
const from = argument(
  "from",
  // `cargo build --target <triple>` puts the binary in target/<triple>/release.
  join(root, "target", triple === `${process.platform}-${process.arch}` ? "" : triple, "release", exe),
);

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
    { version, protocol, target: triple, sha256: createHash("sha256").update(bytes).digest("hex") },
    null,
    2,
  ) + "\n";

const destinations = {
  node: join(root, "packages", "client-node", "bin"),
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
  console.log(`host ${version} (protocol ${protocol}, ${triple}) -> ${destination}`);
}
