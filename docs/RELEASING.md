# Releasing

Host and clients are versioned **in lockstep** (today `0.3.0`): the packages carry the host
binary, so a client version is also a host version. The protocol has its own number
(`PROTOCOL_VERSION` in `packages/host/src/protocol.rs`, today 1) and only changes with an agreed
protocol change.

## What each package contains

| Package | Registry | Contents | Native binary |
|---|---|---|---|
| `gpui-bridge` | npm | `dist/`; the host arrives through the platform package | no |
| `gpui-bridge-react` | npm | `dist/src` only; `gpui-bridge` and `react` are peers | no |
| `@indios/gpui-bridge-<platform>-<arch>` | npm | `bin/gpui-bridge-host[.exe]` + `bin/host.json` | yes, one per platform |
| `gpui-bridge` | PyPI | `gpui_bridge/` (code + `bin/host` + `host.json`) | yes (wheel is platform-tagged) |
| `github.com/InDIOS/gpui-bridge/packages/client-go` | Go module proxy (a git tag) | the source of `packages/client-go/` | no: the host is a release asset |
| GitHub release `v<version>` | GitHub | `gpui-bridge-host-<platform>-<arch>[.exe]` and `SHA256SUMS` | yes, one per platform |

There is one npm platform package per system (`packages/host-<platform>-<arch>`):
`@indios/gpui-bridge-win32-x64`, `@indios/gpui-bridge-darwin-arm64` and `@indios/gpui-bridge-linux-x64`. Their `os` and
`cpu` fields mean npm installs only the one the machine can run, and `gpui-bridge` declares the
three as optional dependencies, so an install downloads a single binary.

Go modules are source only, so a Go program cannot get the host from its dependency: the user
downloads it from the GitHub release of the same version and puts it next to the program or on
`PATH` (the Go client looks there, see its README). Because the module lives in a subdirectory,
its versions are tags named `packages/client-go/v<version>`; nothing in the module carries the
version.

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
   pnpm pack:node                         # packages/client-node/gpui-bridge-0.3.0.tgz
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
5. Publishing is `release.yml`'s job: push the tag `v<version>`. It runs the whole CI on the
   three systems (`ci.yml` as a reusable workflow), then two jobs publish what CI's `pack` job
   built, with GitHub OIDC trusted publishing (no tokens in the repository):

   - `npm`: checks that the tag is the packages' version, then `npm publish` of the three
     platform tarballs first and `gpui-bridge` and `gpui-bridge-react` after them.
   - `pypi`: uploads the three wheels (`pypa/gh-action-pypi-publish`, Linux only), from the
     GitHub environment `pypi`.
   - `github`: creates the GitHub release `v<version>` with the three hosts, renamed
     `gpui-bridge-host-<platform>-<arch>[.exe]`, and `SHA256SUMS`; then pushes the Go module's tag
     `packages/client-go/v<version>` on the same commit and asks the module proxy for it, so
     pkg.go.dev lists it. It uses the workflow's `GITHUB_TOKEN` (no setup).

   Running `release.yml` by hand (*Run workflow*) is the rehearsal: the same CI and artifacts
   (`packages-<platform>-<arch>`), nothing published. A version cannot be published twice on
   either registry, so rehearse before tagging.

The Linux host is built on `ubuntu-22.04`: the glibc it links against (2.35) is the oldest it
runs on, and the wheel's tag (`manylinux_2_35_x86_64`) says so.

## First publication (0.2.0)

npm can only configure a trusted publisher for a package that already exists, so the first
version of each npm package goes out by hand; PyPI accepts a *pending* publisher, so the wheels
can go through `release.yml` from the start. In order:

1. **Make the repository public** (or accept that the package pages link to a repository nobody
   can open, and that npm does not attach provenance to packages from private repositories).
2. **Rehearse:** run `release.yml` by hand on the commit to release and download the
   `packages-win32-x64`, `packages-darwin-arm64` and `packages-linux-x64` artifacts. Check that
   CI was green on the three systems.
3. **npm, by hand, once:** with an npm account with 2FA (`npm login`), from the downloaded
   artifacts, in this order (the platform packages before the client that depends on them):

   ```sh
   npm publish ./host-win32-x64/indios-gpui-bridge-win32-x64-0.2.0.tgz --access public
   npm publish ./host-darwin-arm64/indios-gpui-bridge-darwin-arm64-0.2.0.tgz --access public
   npm publish ./host-linux-x64/indios-gpui-bridge-linux-x64-0.2.0.tgz --access public
   npm publish ./client-node/gpui-bridge-0.2.0.tgz --access public
   npm publish ./client-react/gpui-bridge-react-0.2.0.tgz --access public
   ```

4. **npm trusted publishers:** on npmjs.com, in each of the five packages' *Settings → Trusted
   publisher*, choose GitHub Actions with owner `InDIOS`, repository `gpui-bridge` and workflow
   `release.yml`. From then on, *Publishing access* can be set to require 2FA and disallow
   tokens.
5. **PyPI pending publisher:** on pypi.org, *Your projects → Publishing → Add a new pending
   publisher*: project `gpui-bridge`, owner `InDIOS`, repository `gpui-bridge`, workflow
   `release.yml`, environment `pypi`. In GitHub, create the `pypi` environment (optionally with
   required reviewers, so a tag waits for approval before uploading).
6. **Tag:** `git tag v0.2.0 && git push origin v0.2.0`. The `npm` job skips the versions
   already published in step 3, and the `pypi` job uploads the three wheels.
7. **Acceptance test from the registries,** in a clean folder, without `GPUI_BRIDGE_HOST`:

   ```sh
   npm install gpui-bridge && node -e "import('gpui-bridge').then(m => console.log(m.defaultHostPath()))"
   pip install gpui-bridge && python -c "import gpui_bridge; print(gpui_bridge.default_host_path())"
   ```

   then run the counter with each, and compare `gpui-bridge-host --version` with the installed
   `bin/host.json`.

From 0.2.1 on, a release is steps 2 and 6 only.

## Still open

- **Code signing for the Windows host** (avoids SmartScreen warnings). It needs a certificate or
  a signing service (for example Azure Trusted Signing) that the maintainer owns; until then,
  the binary is unsigned. macOS binaries installed by npm or pip are not quarantined, so
  Gatekeeper does not block them.
- The visual check of a real window per platform is still owed for macOS and Linux (the host is
  only drawn and clicked on Windows so far).
- Only `x64` Windows and Linux and `arm64` macOS have a binary.
- **The hosts in the GitHub release are unsigned.** A macOS host downloaded with a browser is
  quarantined and Gatekeeper blocks it until the user runs `xattr -d com.apple.quarantine` on it
  (signing and notarizing needs an Apple Developer account). The Linux and macOS binaries lose
  their executable bit in the download, so the Go README tells users to `chmod +x` them.
- Package names are `gpui-bridge`, `gpui-bridge-react` and `gpui-bridge` (PyPI) and were free on
  both registries on 2026-10-23. Metadata points at <https://github.com/InDIOS/gpui-bridge>; if
  the organization or repository name changes, update the `package.json` files,
  `pyproject.toml`, the package READMEs and the trusted publishers.
