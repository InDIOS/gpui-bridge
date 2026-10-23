# gpui-bridge-host

The Rust side of gpui-bridge: a GPUI/gpui-component application that owns the windows, the event
loop and the retained tree of UI nodes, and speaks JSON Lines over stdin/stdout with a client in
another language. It is not published on its own: the optimized binary is staged as the asset of
the npm platform package `@indios/gpui-bridge-<platform>-<arch>` and of the Python wheel
(`scripts/stage-host.mjs`). It builds on Windows, macOS and Linux.

## Build

From the repository root (the crate is a workspace member):

```sh
cargo build -p gpui-bridge-host            # debug, used by the clients in a checkout
cargo build -p gpui-bridge-host --release  # what ships (lto, strip, panic=abort)
```

The release profile lives in the workspace root `Cargo.toml`: Cargo ignores `[profile.*]` in a
member, and the packaged host depends on it.

## Run modes

```sh
gpui-bridge-host               # opens a window, reads the protocol from stdin
gpui-bridge-host --no-window   # no platform window: prints the tree to stderr, accepts `simulate`
gpui-bridge-host --version     # version, OS-arch and protocol version (packaging smoke test)
```

stdout is reserved for the protocol; every log goes to stderr. `RUST_LOG=debug` adds per-window
frame and pass timings.

## Tests

```sh
cargo test -p gpui-bridge-host          # unit tests, the conformance transcripts, no_window
cargo test -p gpui-bridge-host --test conformance
```

`tests/conformance.rs` runs every `conformance/*.txt` transcript against the binary in
`--no-window` mode; that directory is shared with the clients, so the wire format stays honest.

## Notes

- The binary asks for an 8 MB main-thread stack (`build.rs`): a debug build spends tens of KB per
  level of a nested tree, and Windows' 1 MB default overflowed (`STATUS_STACK_OVERFLOW`).
- The protocol types are here in `src/protocol.rs`; the specification is `docs/PROTOCOL.md` and
  the Node mirror is `packages/client-node/src/protocol.ts`. A protocol change touches both, the
  other clients, the spec and the transcripts in the same commit.
