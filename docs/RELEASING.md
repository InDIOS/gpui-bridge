# Releasing

Host and clients are versioned **in lockstep** (today `0.1.0`): the packages carry the host
binary, so a client version is also a host version. The protocol has its own number
(`PROTOCOL_VERSION` in `packages/host/src/protocol.rs`, today 1) and only changes with an agreed
protocol change.

## What each package contains

| Package | Registry | Contents | Native binary |
|---|---|---|---|
| `gpui-bridge` | npm | `dist/`; the host arrives through the platform package | no |
| `gpui-bridge-react` | npm | `dist/src` only; `gpui-bridge` and `react` are peers | no |
| `gpui-bridge-<platform>-<arch>` | npm | `bin/gpui-bridge-host[.exe]` + `bin/host.json` | yes, one per platform |
| `gpui-bridge` | PyPI | `gpui_bridge/` (code + `bin/host` + `host.json`) | yes (wheel is platform-tagged) |

There is one npm platform package per system (`packages/host-<platform>-<arch>`):
`gpui-bridge-win32-x64`, `gpui-bridge-darwin-arm64` and `gpui-bridge-linux-x64`. Their `os` and
`cpu` fields mean npm installs only the one the machine can run, and `gpui-bridge` declares the
three as optional dependencies, so an install downloads a single binary.

`bin/host.json` carries `{ version, protocol, target, sha256 }`; it is what a bug report should
quote, and `gpui-bridge-host --version` prints the same facts.

## Steps

1. Bump the version in one place and propagate it (`Cargo.toml` workspace, the four
   `package.json`, `pyproject.toml`), update `CHANGELOG.md`, commit.
2. Build the host **once** for each platform you ship:

   ```sh
   cargo build -p gpui-bridge-host --release
   node scripts/stage-host.mjs            # -> the platform package + the wheel, with host.json
   ```

   For a cross target: `cargo build -p gpui-bridge-host --release --target <triple>` and
   `node scripts/stage-host.mjs --triple <triple>` (the tag is derived from the triple; `--tag`
   overrides it). `prepack` and the wheel builder only copy the binary: nothing compiles twice.
3. Pack the client, the platform package for this machine and the platform-tagged wheel:

   ```sh
   pnpm pack:node                         # packages/client-node/gpui-bridge-0.1.0.tgz
   pnpm --dir packages/host-<platform>-<arch> pack
   pnpm wheel                             # -> packages/client-python/dist/*.whl
   ```

4. Check the artifacts, and run the acceptance test:

   ```sh
   node scripts/check-package.mjs         # or: pnpm check:package
   ```

   It installs the tarballs and the wheel in a folder **outside the repository**, with no
   `GPUI_BRIDGE_HOST`, runs a headless app with the Node and the Python client and reports which
   host each one resolved: the host that runs is the one the packages carry (this is the
   phase-16 acceptance test).
5. Publish the platform package **before** the client that depends on it, then the rest:

   ```sh
   pnpm --dir packages/host-<platform>-<arch> publish --no-git-checks
   pnpm --dir packages/client-node publish --no-git-checks
   pnpm --dir packages/client-react publish --no-git-checks
   twine upload packages/client-python/dist/*.whl
   ```

   Tag `v<version>` afterwards.

CI (`release.yml`) does 1–4 for tags and publishes when `NPM_TOKEN` and `TWINE_*` secrets exist.

## Before the first public release

- Package names are `gpui-bridge`, `gpui-bridge-react` and `gpui-bridge` (PyPI): renaming them is a
  breaking change for every README and example, so decide now.
- Metadata is already pointed at <https://github.com/InDIOS/gpui-bridge> (`repository`,
  `homepage`, `bugs` in the `package.json`, `[project.urls]` in `pyproject.toml`). If the
  organization or repository name changes, update those files plus the package READMEs.
- The host builds and its tests run on Windows, macOS and Linux (the CI matrix), and the
  packaging of each platform is verified there (tarball, wheel, install and headless run). What
  is still Windows-only is *publishing*: `release.yml` runs on one runner, so M6 has to turn it
  into a matrix that publishes the three platform packages and the three wheels of a release.
- Code signing for the Windows host avoids SmartScreen warnings for users.
- The visual check of a real window per platform is still owed for macOS and Linux (the host is
  only drawn and clicked on Windows so far).
