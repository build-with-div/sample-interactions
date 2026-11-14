# Sample Interactions

A dogfood sample for [Div](https://github.com/build-with-div/div), a
native Rust UI framework that aims to render HTML and CSS as Chromium does.
It ports the npm packages that desktop apps use for interaction and data
views to Rust on Div, and finds the web platform features Div must
implement to run them faithfully. The packages are dnd-kit, react-dnd,
react-resizable-panels, Radix UI and Floating UI, cmdk, TanStack Virtual
and Table, AG Grid (as a reference) and react-arborist.

Status: phase 2, the ports. [docs/design.md](docs/design.md) maps each
package to the platform features it uses, says what Div has, what is
missing, the owner and the size, and lists the port milestones.
[docs/rfc-interactions.md](docs/rfc-interactions.md) drafts those features
as a Div RFC, and [docs/gaps.md](docs/gaps.md) lists every `div-gap:`
scaffold in the code with the feature that replaces it. Each port is
compared with the original package in pinned Chromium.

## What is here

- `src/panels`: react-resizable-panels 4.22.1 on Div (`Group`, `Panel`,
  `Separator`). `tests/panels_oracle.rs` checks its layout functions
  against the package's own, and `tests/panels_group.rs` drives it on Div.
- `src/table`: `@tanstack/table-core` 9.3.1's model (columns, rows,
  header groups, sorting, column sizing and resizing) in Rust.
  `tests/table_oracle.rs` checks every result against the package.
- The fidelity runner: `tests/fidelity_panels.rs` replays scripted
  scenarios on Div and compares each step with the package itself in
  pinned Chromium (`reference/` builds the page, `tools/fidelity/` drives
  Chromium). The measured differences are in
  `tests/fixtures/fidelity/panels-div-differences.txt`.
- `tests/probe_*.rs`: headless probes. Each asserts what Div does today at
  the pinned commit and prints a `PROBE <id>: ...` line.
- `src/bin/native_probe.rs` and `tools/native-input.ps1`: a real window
  driven by posted Win32 mouse messages, which show what the native
  adapter sends.

## Build and run

Div is not on crates.io. `Cargo.toml` pins it to a commit of
build-with-div/div, and `Cargo.lock` is seeded from that commit's lock (Div
gap: a fresh resolve does not build on Linux). Div compiles Skia from
source, which needs:

- Rust 1.94.1 (`rust-toolchain.toml` selects it), Python 3, Ninja and LLVM
  (clang and libclang).
- Windows: Visual Studio 2022 Build Tools with the C++ workload. Run cargo
  from a shell where `vcvars64.bat` has been applied, with `LIBCLANG_PATH`
  pointing at LLVM's `bin`.
- Linux (Ubuntu 24.04): the packages installed by
  [.github/workflows/ci.yml](.github/workflows/ci.yml), and `CC=clang
  CXX=clang++ CLANGCC=clang CLANGCXX=clang++`.

`.cargo/config.toml` sets Div's Skia build variables. Then:

```sh
cargo test -- --nocapture --test-threads=1
```

To regenerate the react-resizable-panels oracle (Node 22.18 or newer):

```sh
git clone --depth 1 --branch 4.22.1 https://github.com/bvaughn/react-resizable-panels out/rrp-4.22.1
node tools/rrp-oracle/generate.mjs out/rrp-4.22.1 tests/fixtures/rrp-oracle.json
```

To regenerate the TanStack Table oracle from the published package:

```sh
cd reference && npm ci && cd ..
node tools/table-oracle/generate.mjs tests/fixtures/table-oracle.json
```

To recapture Chromium for the fidelity runner, with `CHROME` the `chrome`
binary of Chrome for Testing 147.0.7727.50 (the build Div's census pins):

```sh
cd reference && npm ci && node build.mjs && cd ..
node tools/fidelity/capture.mjs CHROME tests/fixtures/fidelity/panels-chromium.json
node tools/fidelity/flex-grow-probe.mjs CHROME tests/fixtures/fidelity/flex-grow-chromium.json
```

On Windows, `pwsh tools/native-input.ps1` (after `cargo build --bin
native_probe`) runs the native probe. It opens a window for about three
seconds.
