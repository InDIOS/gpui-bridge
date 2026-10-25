# Changelog

Versions are lockstep across the host and the three clients; the protocol has its own version
(`PROTOCOL_VERSION`, currently 1).

## Unreleased

- Go client (`packages/client-go/`, module `github.com/InDIOS/gpui-bridge/packages/client-go`,
  package `gpuibridge`), standard library only. Its first version is tagged
  `packages/client-go/v<version>` by `release.yml`.
- The GitHub release of each version carries the three host binaries and `SHA256SUMS`: Go
  modules are source only, so a Go program downloads its host from there.
- PROTOCOL.md: a rejected `create` leaves no node (§4); what a client checks against the
  transcripts (§9).
- New node `Canvas`: the client draws with `rect`, `line` and `path` commands in theme colors, and
  gets `pointerDown` with the point local to the canvas (PROTOCOL.md §20). `canvas()` in Node,
  `<Canvas onPointerDown>` in React, `app.canvas()` in Python and `App.Canvas` in Go.
- New node `Command`: a command palette over sections of entries (Sidebar's shape plus
  `keywords` and `checked`); it filters by the query itself and reports `confirm` with the
  entry's `id` (PROTOCOL.md §21). Inside a `Dialog` it is a modal palette. `command()` in Node,
  `<Command onConfirm>` in React, `app.command()` in Python and `App.Command` in Go.

## 0.2.0 — first public release

- First publication: npm (`gpui-bridge`, `gpui-bridge-react` and the three platform packages)
  and PyPI (`gpui-bridge`, one wheel per platform: Windows x64, macOS arm64, Linux x64).
- Window props: initial position (`x`, `y`), minimum size (`minWidth`, `minHeight`), `minimized`, and
  opt-in window events (`focus`, `blur`, `resize`, `move`, `visibility`).
- New nodes: `Tree`, `Diff`, `Carousel`, `Sidebar`, `Chart` (`line`, `bar`, `pie`, with `xTicks`
  and `yTicks`) and `Tab` inside `Tabs` (tabs made of nodes; `Tabs` without children works as
  before). `Tabs` takes the `folder` variant.
- The npm platform packages are scoped: `@indios/gpui-bridge-win32-x64`,
  `@indios/gpui-bridge-darwin-arm64` and `@indios/gpui-bridge-linux-x64` (npm's spam filter
  rejected the unscoped names). `gpui-bridge` and `gpui-bridge-react` keep their names.
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
