# Contributing

The repository is a monorepo: one Rust host and three clients (Node, React, Python) that talk to
it over a documented wire protocol. The host binary is shipped as an asset of the packages (an npm
platform package per system, and the platform-tagged Python wheel), so packaging is part of the
build, not an afterthought.

## Get it running

Requirements: Rust (stable, with `rustup`), the platform's C/C++ toolchain (Visual Studio Build
Tools on Windows, Xcode on macOS, Vulkan/Wayland/X11 libraries on Linux), Node.js 22.6+ with pnpm,
and Python 3.10+ for the Python client. The first build of GPUI takes several minutes.

```sh
pnpm bootstrap                 # pnpm install + debug host + TypeScript builds
pnpm dev counter               # opens the counter example
pnpm test                      # host, Node, React and Python
```

`pnpm dev <name>` resolves the host and sets `GPUI_BRIDGE_HOST` for the example: `pnpm dev todo`,
`pnpm dev react/todo`, `pnpm dev counter.py`. It prefers the debug build in `target/debug` and
falls back to `target/release`; `--debug` prints which host it runs and forwards its logs, e.g.
`pnpm dev forms --debug`. Extra arguments go to the example (pnpm swallows a `--`, so both
`--debug` and `-- --debug` work).

## Layout

| Path | What lives there |
|---|---|
| `packages/host/` | the Rust host (workspace member), its unit tests and the transcript runner |
| `packages/client-node/` | npm `gpui-bridge`; also the txiki.js entry point |
| `packages/client-react/` | npm `gpui-bridge-react` (peer of `gpui-bridge`) |
| `packages/client-python/` | PyPI `gpui-bridge` (setuptools, not part of the pnpm workspace) |
| `packages/host-<platform>-<arch>/` | the npm platform package that carries the host binary |
| `examples/` | runnable examples; they import the packages by name, like a user |
| `conformance/` | wire-level transcripts, shared by the host and every client |
| `benchmarks/` | measurements, not tests |
| `docs/` | protocol, plan, history, releasing, ADRs |
| `scripts/` | `stage-host.mjs` (packaging), `check-package.mjs` (the packaging test) and `dev.mjs` (`pnpm dev`) |

The root `Cargo.toml` holds the Cargo workspace and `[profile.release]` (a profile in a member is
ignored); `pnpm-workspace.yaml` holds the pnpm workspace and the only lockfile.

## Ground rules

1. **stdout is the protocol.** In the host, never print to stdout outside `transport.rs`; logs go
   to stderr.
2. **The host never dies on a bad message.** Answer with `error` and keep going.
3. **The protocol has two sources of truth** (`packages/host/src/protocol.rs` and
   `packages/client-node/src/protocol.ts`) plus the other clients. A protocol change updates all of
   them, `docs/PROTOCOL.md` and `conformance/` in the same commit, and is agreed before coding.
4. **Small, verifiable commits** on the local `dev` branch, one per step. Note each finished step
   in `docs/HISTORIAL.md`.
5. **Test in a real window.** Automated tests run with `--no-window`; what is drawn, clicked or
   focused is verified by opening the example, and the history says what was and was not checked.

## Checks before a commit

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
pnpm test
pnpm --filter gpui-bridge-examples typecheck
```

CI (`.github/workflows/ci.yml`) runs exactly these on Windows, macOS and Linux (the host) and on
Windows and Linux (the clients), plus the packaging job per platform, on the Rust version pinned
in `rust-toolchain.toml`.

Packaging changes also need the three artifacts (`pnpm pack:node`, `pnpm --dir
packages/host-<platform>-<arch> pack`, `pnpm wheel`) and `pnpm check:package`, which installs
them **outside the repository** and runs a headless app with both clients, without
`GPUI_BRIDGE_HOST`.
