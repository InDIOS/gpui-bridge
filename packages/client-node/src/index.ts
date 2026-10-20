// Public API of the gpui-bridge Node client.

import { existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { connect, type App } from "./app.js";
import { HostProcess } from "./host-process.js";

export * from "./app.js";
export * from "./protocol.js";

export interface AppOptions {
  /** Host binary. Defaults to `$GPUI_BRIDGE_HOST`, then the host bundled in this package, then the debug build in this repository. */
  hostPath?: string;
  /** Forward host logs (stderr) to the console. */
  debug?: boolean;
  /** Run the host without a window (`--no-window`), e.g. for tests. */
  headless?: boolean;
}
const exe = process.platform === "win32" ? "gpui-bridge-host.exe" : "gpui-bridge-host";
// dist/index.js -> the package root
const pkg = join(dirname(fileURLToPath(import.meta.url)), "..");

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

function defaultHostPath(): string {
  if (process.env.GPUI_BRIDGE_HOST) return process.env.GPUI_BRIDGE_HOST;
  // `pnpm pack` puts the optimized host in bin/ (scripts/stage-host.mjs); a checkout has none.
  const bundled = join(pkg, "bin", exe);
  return existsSync(bundled) ? bundled : (repoTarget() ?? join(pkg, "..", "..", "..", "target", "debug", exe));
}

/** Starts the host and completes the handshake. Rejects with a clear message if the host cannot start. */
export function createApp(options: AppOptions = {}): Promise<App> {
  const path = options.hostPath ?? defaultHostPath();
  const args = options.headless ? ["--no-window"] : [];
  return connect(path, (events) => new HostProcess({ path, args, debug: options.debug ?? false, ...events }));
}
