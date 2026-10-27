## What this changes

<!-- One paragraph: the step (phase/hito), and what a user of the library notices. -->

## Checklist

- [ ] One step only, on `dev`, with a descriptive message (CONTRIBUTING.md, ground rules).
- [ ] If the protocol changed: `packages/host/src/protocol.rs`, `packages/client-node/src/protocol.ts`,
      the other clients, `docs/PROTOCOL.md` and `conformance/` are all in this change, and the
      change was agreed first.
- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `pnpm test`
- [ ] `pnpm --filter gpui-bridge-examples typecheck`
- [ ] Packaging changes: `pnpm pack:node` and `pnpm wheel`, installed **outside** the repository.
- [ ] What was tested in a real window (and what was not) is written in `docs/HISTORIAL.md`.

## Notes

<!-- Known gaps, follow-ups, or anything a reviewer should look at twice. -->
