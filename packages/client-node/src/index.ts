// Public API of the gpui-bridge Node client.

import { existsSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { connect, type App } from "./app.js";
import { HostProcess } from "./host-process.js";

export * from "./app.js";
export * from "./protocol.js";

export interface AppOptions {
  /** Host binary. Defaults to `$GPUI_BRIDGE_HOST`, then the platform package installed with this one, then the debug build in this repository. */
  hostPath?: string;
  /** Forward host logs (stderr) to the console. */
  debug?: boolean;
  /** Run the host without a window (`--no-window`), e.g. for tests. */
  headless?: boolean;
}
const exe = process.platform === "win32" ? "gpui-bridge-host.exe" : "gpui-bridge-host";
// dist/index.js -> the package root
const pkg = join(dirname(fileURLToPath(import.meta.url)), "..");
const nodeRequire = createRequire(import.meta.url);
// The platform the host must run on, and the suffix of the package that ships it.
const platform = `${process.platform}-${process.arch}`;

/**
 * Walks up from this file looking for a Cargo build of the host. The number of folders
 * between a client and the workspace root is not part of the contract, so this survives
 * a client moving inside `packages/` (or anywhere else).
 */
function repoTarget(): string | undefined {
  let dir = dirname(fileURLToPath(import.meta.url));
  for (let depth = 0; depth < 6; depth++) {
    for (const profile of ["debug", "release"]) {
      const candidate = join(dir, "target", profile, exe);
      if (existsSync(candidate)) return candidate;
    }
    dir = dirname(dir);
  }
  return undefined;
}

/**
 * The binary of `gpui-bridge-<platform>-<arch>`, an optional dependency of this package:
 * npm and pnpm install only the one whose `os`/`cpu` match the machine, so whatever is
 * installed here is a host this machine can run. That package carries no code, only the
 * asset, so it is found through its manifest (it declares no `exports` to walk).
 */
function packagedHost(): string | undefined {
  try {
    const manifest = nodeRequire.resolve(`gpui-bridge-${platform}/package.json`);
    const candidate = join(dirname(manifest), "bin", exe);
    return existsSync(candidate) ? candidate : undefined;
  } catch {
    return undefined; // another platform, or a checkout where the asset was never staged
  }
}

/** The host to run: `$GPUI_BRIDGE_HOST`, then the packaged one, then a build in this repository. */
export function defaultHostPath(): string {
  return process.env.GPUI_BRIDGE_HOST ?? packagedHost() ?? repoTarget() ?? join(pkg, "..", "..", "..", "target", "debug", exe);
}

/** Starts the host and completes the handshake. Rejects with a clear message if the host cannot start. */
export function createApp(options: AppOptions = {}): Promise<App> {
  const path = options.hostPath ?? defaultHostPath();
  const args = options.headless ? ["--no-window"] : [];
  return connect(path, (events) => new HostProcess({ path, args, debug: options.debug ?? false, ...events }));
}
