# Changelog

Versions are lockstep across the host and the three clients; the protocol has its own version
(`PROTOCOL_VERSION`, currently 1).

## 0.2.0 — first public release

- First publication: npm (`gpui-bridge`, `gpui-bridge-react` and the three platform packages)
  and PyPI (`gpui-bridge`, one wheel per platform: Windows x64, macOS arm64, Linux x64).
- Windows: initial position (`x`, `y`), minimum size (`minWidth`, `minHeight`), `minimized`, and
  opt-in window events (`focus`, `blur`, `resize`, `move`, `visibility`).
- New nodes: `Tree`, `Diff`, `Carousel`, `Sidebar`, `Chart` (`line`, `bar`, `pie`, with `xTicks`
  and `yTicks`) and `Tab` inside `Tabs` (tabs made of nodes; `Tabs` without children works as
  before). `Tabs` takes the `folder` variant.
- The host is built on gpui-kit 0.8.
- The Linux wheel is tagged with the glibc it was built against (`manylinux_2_35_x86_64`), not
  `manylinux2014`, which promised a glibc the host does not run on.
- `release.yml` runs the whole CI on the three systems and publishes with GitHub OIDC trusted
  publishing (no npm or PyPI tokens).
- Protocol version stays at 1.

## 0.1.0 — repository layout

- Monorepo: Cargo workspace at the root (with `[profile.release]`), pnpm workspace with a single
  lockfile, `packages/` for the host and the clients, `docs/` for the documents.
- Host compiled once and staged into the packages that ship it (`scripts/stage-host.mjs`,
  `bin/host.json`); `prepack` and the wheel builder no longer compile.
- Clients resolve the host by walking up to `target/{debug,release}`, so moving a package cannot
  break them; `gpui-bridge-host --version` added for packaging checks.
- One npm package per platform (`gpui-bridge-win32-x64`, `gpui-bridge-darwin-arm64`,
  `gpui-bridge-linux-x64`) carries the host binary; `gpui-bridge` pulls the one for the machine
  in as an optional dependency and its clients declare no `os`/`cpu`. `defaultHostPath()`
  reports the host a Node app resolved.
- Python wheels are tagged with the platform they carry their host for (`py3-none-win_amd64`,
  `py3-none-macosx_11_0_arm64`, `py3-none-manylinux2014_x86_64`), and the package metadata has a
  README, license and classifiers for the three systems.
- The host builds and its tests run on Windows, macOS and Linux (CI matrix of the three).
- Examples consume the packages by name through the workspace; React examples moved to
  `examples/react/`; `pnpm dev <name>` runs any example with the repository host.
- Docs: `CONTRIBUTING.md`, `docs/RELEASING.md`, ADR 0001 and a README per package.

Earlier development is described phase by phase in [`docs/HISTORIAL.md`](docs/HISTORIAL.md).
