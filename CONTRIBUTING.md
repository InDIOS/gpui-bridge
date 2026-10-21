# Contributing

The repository is a monorepo: one Rust host and three clients (Node, React, Python) that talk to
it over a documented wire protocol. The host binary is shipped inside the client packages as an
asset, so packaging is part of the build, not an afterthought.

## Get it running

Requirements: Rust (stable, with `rustup`), Visual Studio Build Tools with the C++ workload,
Node.js 22.6+ with pnpm, and Python 3.10+ for the Python client. The first build of GPUI takes
several minutes.

```sh
pnpm bootstrap                 # pnpm install + debug host + TypeScript builds
pnpm dev counter               # opens the counter example
pnpm test                      # host, Node, React and Python
```

`pnpm dev <name>` resolves the host and sets `GPUI_BRIDGE_HOST` for the example: `pnpm dev todo`,
`pnpm dev react/todo`, `pnpm dev counter.py`. Extra arguments after `--` go to the example.

## Layout

| Path | What lives there |
|---|---|
| `packages/host/` | the Rust host (workspace member), its unit tests and the transcript runner |
| `packages/client-node/` | npm `gpui-bridge`; also the txiki.js entry point |
| `packages/client-react/` | npm `gpui-bridge-react` (peer of `gpui-bridge`) |
| `packages/client-python/` | PyPI `gpui-bridge` (setuptools, not part of the pnpm workspace) |
| `examples/` | runnable examples; they import the packages by name, like a user |
| `conformance/` | wire-level transcripts, shared by the host and every client |
| `benchmarks/` | measurements, not tests |
| `docs/` | protocol, plan, history, releasing, ADRs |
| `scripts/` | `stage-host.mjs` (packaging) and `dev.mjs` (`pnpm dev`) |

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

CI (`.github/workflows/ci.yml`) runs exactly these plus the packaging job, on the Rust version
pinned in `rust-toolchain.toml`.

Packaging changes also need `pnpm pack:node` and `pnpm wheel`, then installing the tarball and the
wheel **outside the repository** and running an example without `GPUI_BRIDGE_HOST`.
