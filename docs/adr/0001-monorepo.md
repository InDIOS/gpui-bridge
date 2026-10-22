# ADR 0001 — Monorepo layout and how the host travels with each client

Status: accepted (applied on the `dev` branch).

## Context

One Rust host and three clients (Node, React, Python). Each published client must work after
`npm install` / `pip install` with no Rust toolchain, so the host binary is an **asset inside the
package**. The repository also has to be easy to pick up: clone, one command, one test command.

## Decisions

1. **Root is an orchestrator.** `Cargo.toml` (workspace + `[profile.release]`),
   `pnpm-workspace.yaml` (the only lockfile), root `package.json` scripts, and `scripts/`.
   `packages/` holds everything distributable; `docs/`, `conformance/`, `benchmarks/`, `examples/`
   stay at the root because they span packages.
2. **The release profile lives in the workspace root.** Cargo silently ignores `[profile.*]` in a
   member; the packaged host depends on `lto`, `strip` and `panic = "abort"`.
3. **The host is built once and staged.** `scripts/stage-host.mjs` copies
   `target/release/gpui-bridge-host[.exe]` into `packages/host-<platform>-<arch>/bin/` (the npm
   package for the platform it was built for) and `packages/client-python/gpui_bridge/bin/`, plus
   a `host.json` with version, protocol, target and hash. `prepack` and `build_wheel.py` never
   compile: they fail with instructions if the asset is missing. CI passes the same binary
   between jobs as an artifact, one per platform.
4. **React ships no binary.** It declares `gpui-bridge` as a peer dependency; duplicating a 15 MB
   host would create two binaries in one install.
5. **Clients find the host by walking up**, not by counting `..`: explicit option,
   `$GPUI_BRIDGE_HOST`, the asset of the platform package installed with the client, then
   `target/{debug,release}` upwards. The number of folders between a client and the workspace
   root is not part of the contract.
6. **Examples consume the packages by name** (`import { createApp } from "gpui-bridge"`), through
   the pnpm workspace, so they mirror what a user installs. React examples live in
   `examples/react/`, not inside the React package.
7. **Package names stay** (`gpui-bridge`, `gpui-bridge-react`): renaming would break the README,
   the peer dependency and every example. A scope, if wanted, is decided before the first publish.
8. **Python stays outside the JS workspace.** setuptools builds it; it is only a sibling folder.

## Consequences

- One `cargo build` and one `pnpm install` at the root; `pnpm bootstrap`, `pnpm dev`, `pnpm test`.
- `docs/HISTORIAL.md` keeps its historical paths (it is a log); this ADR explains the current
  layout, and `docs/GPUI_BRIDGE_PLAN.md` §4 is the reference tree.
- Future packages (`packages/protocol`, per-platform host packages) fit without moving anything
  again.
