# Sample Interactions

A dogfood sample for [Div](https://github.com/build-with-div/div), a
native Rust UI framework that aims to render HTML and CSS as Chromium does.
It ports the npm packages that desktop apps use for interaction and data
views to Rust on Div, and finds the web platform features Div must
implement to run them faithfully. The packages are dnd-kit, react-dnd,
react-resizable-panels, Radix UI and Floating UI, cmdk, TanStack Virtual
and Table, AG Grid (as a reference) and react-arborist.

Status: phase 1, probes and design. [docs/design.md](docs/design.md) maps
each package to the platform features it uses and says what Div has, what
is missing, the owner and the size.
[docs/rfc-interactions.md](docs/rfc-interactions.md) drafts those features
as a Div RFC. The ports come after review, each compared with the original
package in pinned Chromium.

## What is here

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

On Windows, `pwsh tools/native-input.ps1` (after `cargo build --bin
native_probe`) runs the native probe. It opens a window for about three
seconds.
