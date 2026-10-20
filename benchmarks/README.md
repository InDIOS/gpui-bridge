# Benchmarks

Numbers for the transport and rendering decisions (protocol shape, big trees, image payloads).
They are not a test suite: nothing here asserts, it only measures and prints tables.

## Running

```sh
pnpm bootstrap                              # install + debug host + TypeScript builds

pwsh -File benchmarks/run.ps1                    # everything, one table per bench
pwsh -File benchmarks/run.ps1 -Only render       # tree | micro | render | payload | images | all
pwsh -File benchmarks/run.ps1 -Only images -Images C:\tmp\a.jpg,C:\tmp\b.jpg
```

Each bench can also be run alone:

```sh
node benchmarks/tree.mjs 1000 10000 50000        # transport and tree apply (--no-window)
node --expose-gc benchmarks/payload-micro.mjs    # JSON floor: stringify/parse of one line
node benchmarks/render.mjs 50000 spin            # builds the window; run.ps1 samples the host
node benchmarks/payload.mjs 4194304 retained     # sends one payload; run.ps1 samples the host
node benchmarks/payload.mjs 0 image <file.jpg>   # a real image by path; the decode cost
```

`GPUI_BRIDGE_HOST` overrides the host binary. `run.ps1` samples the host that appeared after the
bench client started, so other hosts running on the machine do not interfere.

Two more scripts take one sample by hand, for the checks that are not a table:

```sh
pwsh -File benchmarks/sample.ps1 -Nodes 20000 -Mode spin -Seconds 5 -Log   # one render case, with the host's frames/s
pwsh -File benchmarks/hover-check.ps1                                     # a real pointer over a `hover:` transition
pwsh -File benchmarks/hover-check.ps1 -Example examples/data.ts -Width 720 -Height 560 -AtX 300 -AtY 150
```

`hover-check.ps1` opens an example and moves the real pointer to a point inside its window (the
window opens centered, so the point is given as an offset inside it), which is the only way to
exercise a `hover:` from outside; the pointer goes back where it was. It reads the host's frames/s
to tell a fade that animates from one that does not.

## What each one measures

**`tree.mjs`** — writes N `create` + `append` operations to `host --no-window`, commits, and then
measures the round trip of a `simulate` click, which can only come back after the commit was
applied. The same round trip is measured again with the host idle; the difference is the reported
`apply ms`. It says nothing about drawing. Timings are client-side wall clock, so they include
process scheduling.

**`payload-micro.mjs`** — `JSON.stringify` and `JSON.parse` of one line carrying a base64 value, no
host involved: the floor that the numbers from `payload.mjs` sit on top of.

**`render.mjs` + `run.ps1`** — a window with N nodes, sampled for 5 s of host CPU with the tree
quiet (`idle`) and with one spinning icon (`spin`, which keeps the window drawing at 60 frames a
second). CPU is `TotalProcessorTime` over the sample, as a percentage of one core; the client
prints `READY` only after the first build settled, so the sample is steady state.

Because the spinner keeps drawing, `spin` also measures the platform: on the machine the numbers
below were taken from, a frame of a small window already costs a few milliseconds, so the host can
sit at one core while its own pass is a fraction of a millisecond. Run the host with
`RUST_LOG=debug` (`bench/sample.ps1 -Log`) to see the two separately: `window 1: 60.0 frames/s,
0.2 ms and 14 nodes per pass` is the host's work, the rest is GPUI drawing and presenting.

**`payload.mjs` + `run.ps1`** — one large value in one protocol line, and the host's working set and
peak working set after it was applied:

| mode | what it sends |
|---|---|
| `baseline` | a one-character value, to see a host with no payload |
| `retained` | a base64 value of N bytes, kept in the tree (the worst case for the "bytes in a line" path) |
| `cleared` | the same, then emptied, to separate the transient cost from the retained one |
| `path` | the bytes written to a temp file and a short path sent instead (the "client downloads, host reads a file" path) |
| `image` | a real image file shown as an `Image` node (takes the path as a fourth argument): the decode cost, which no transport change moves |

**Known bias:** `Input.value` is a string the node keeps **and lays out as text**, so the
`retained`/`cleared` rows measure text shaping of a huge string more than transport (64 KB →
+4 MB, 1 MB → +21 MB, and non-monotonic values at 4 MB; measured 2026-10-19). Read them together
with `payload-micro.mjs` (the encoding floor) and with `image` (the decode cost). The `path` row
is the no-bytes baseline.

Numbers are from the machine they were run on and are only comparable to each other; the tables
recorded in the project's history say which machine and when.