# Releasing

Host and clients are versioned **in lockstep** (today `0.1.0`): the npm and Python packages carry
the host binary, so a client version is also a host version. The protocol has its own number
(`PROTOCOL_VERSION` in `packages/host/src/protocol.rs`, today 1) and only changes with an agreed
protocol change.

## What each package contains

| Package | Registry | Contents | Native binary |
|---|---|---|---|
| `gpui-bridge` | npm | `dist/` + `bin/gpui-bridge-host.exe` + `bin/host.json` | yes (Windows x64) |
| `gpui-bridge-react` | npm | `dist/src` only; `gpui-bridge` and `react` are peers | no |
| `gpui-bridge` | PyPI | `gpui_bridge/` (code + `bin/host` + `host.json`) | yes (wheel is platform-tagged) |

`bin/host.json` carries `{ version, protocol, target, sha256 }`; it is what a bug report should
quote, and `gpui-bridge-host --version` prints the same facts.

## Steps

1. Bump the version in one place and propagate it (`Cargo.toml` workspace, both `package.json`,
   `pyproject.toml`), update `CHANGELOG.md`, commit.
2. Build the host **once** for each platform you ship:

   ```sh
   cargo build -p gpui-bridge-host --release
   node scripts/stage-host.mjs            # node + python assets, with host.json
   ```

   For a cross target: `cargo build -p gpui-bridge-host --release --target <triple>` and
   `node scripts/stage-host.mjs --triple <triple>`.
3. Check the asset and the package metadata:

   ```sh
   packages/client-node/bin/gpui-bridge-host.exe --version
   pnpm --filter gpui-bridge pack         # prepack rebuilds tsc, stages and packs
   python packages/client-python/build_wheel.py
   ```

4. Install the tarball and the wheel in a folder **outside the repository**, with no
   `GPUI_BRIDGE_HOST`, and run an example (this is the phase-16 acceptance test).
5. Publish: `pnpm --filter gpui-bridge publish`, then `pnpm --filter gpui-bridge-react publish`,
   then `twine upload packages/client-python/dist/*.whl`. Tag `v<version>` afterwards.

CI (`release.yml`) does 1–4 for tags and publishes when `NPM_TOKEN` and `TWINE_*` secrets exist.

## Before the first public release

- Package names are `gpui-bridge`, `gpui-bridge-react` and `gpui-bridge` (PyPI): renaming them is a
  breaking change for every README and example, so decide now.
- Metadata is already pointed at <https://github.com/InDIOS/gpui-bridge> (`repository`,
  `homepage`, `bugs` in both `package.json`, `[project.urls]` in `pyproject.toml`). If the
  organization or repository name changes, update those three files plus the package READMEs.
- The host is Windows-only today. Adding macOS/Linux means a host build per platform, one asset
  folder per platform in `bin/` (the client already resolves `bin/<platform>-<arch>/`), and
  per-platform wheel tags. Consider per-platform npm packages (`optionalDependencies`) when the
  single tarball would have to carry more than one binary.
- The npm packages declare `os: ["win32"]` and `cpu: ["x64"]`; relax those together with the new
  assets.
- Code signing for the Windows host avoids SmartScreen warnings for users.
