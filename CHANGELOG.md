# Changelog

Versions are lockstep across the host and the three clients; the protocol has its own version
(`PROTOCOL_VERSION`, currently 1).

## 0.1.0 — repository layout

- Monorepo: Cargo workspace at the root (with `[profile.release]`), pnpm workspace with a single
  lockfile, `packages/` for the host and the clients, `docs/` for the documents.
- Host compiled once and staged into the npm and Python packages (`scripts/stage-host.mjs`,
  `bin/host.json`); `prepack` and the wheel builder no longer compile.
- Clients resolve the host by walking up to `target/{debug,release}`, so moving a package cannot
  break them; `gpui-bridge-host --version` added for packaging checks.
- Python wheel is tagged with its platform (`py3-none-win_amd64`) instead of `py3-none-any`, and
  the package metadata now has a README, license and classifiers.
- npm packages declare `os: ["win32"]`, `cpu: ["x64"]` and Node 22.6+.
- Examples consume the packages by name through the workspace; React examples moved to
  `examples/react/`; `pnpm dev <name>` runs any example with the repository host.
- Docs: `CONTRIBUTING.md`, `docs/RELEASING.md`, ADR 0001 and a README per package.

Earlier development is described phase by phase in [`docs/HISTORIAL.md`](docs/HISTORIAL.md).
