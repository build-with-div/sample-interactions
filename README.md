# Sample Interactions

A dogfood sample for [Div](https://github.com/build-with-div/div), a
native Rust UI framework. It covers the interaction patterns and data views
that professional desktop apps are built from: drag handles, split panes,
sortable lists and kanban drags, OS drag and drop, pointer, wheel and
keyboard input, focus management, overlays, virtualized lists, trees and
data grids, and window chrome. It uses only Div's public API, and part of
its job is to find what Div is still missing.

Status: phase 1, probes and design. Read [docs/design.md](docs/design.md)
for what works, what needs a workaround and what is missing, pattern by
pattern. [docs/rfc-interactions.md](docs/rfc-interactions.md) drafts the
primitives Div could add. The workbench app comes after review.

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
