# Contributing

Thanks for your interest in Almondbread. Bug reports, ideas and pull requests are welcome.

- **Issues:** describe what you saw and how to get there. For rendering problems, the
  coordinates help most: *Copy Coordinates* (Shift-Cmd-C) puts the centre and scale on the
  clipboard.
- **Pull requests:** keep them focused; one change per PR. Open an issue first for larger
  changes, so we can agree on the approach.
- **Licence:** contributions are accepted under the MIT licence of this repository
  (see `LICENSE`). By opening a pull request you confirm you have the right to submit the
  code under it. Do not copy code from projects with incompatible licences (for example the
  AGPL fractal programs); ideas and published formulas are fine, cite them.

## Layout

The renderer, maths, navigation and scheduler are Rust (`crates/`); the macOS and iOS UI is
SwiftUI (`Sources/`, `Apps/`). `CLAUDE.md` describes the layout and the tools in detail, and
`kb/` holds the background (techniques, decisions, benchmarks).

## Checks

Run these before opening a pull request:

```sh
make rust-check   # cargo test (release), clippy -D warnings, cargo fmt --check
make build        # Swift package build
make app          # Xcode build of the macOS app (needs xcodegen)
make selftest     # renders the first view in a background window; prints "renderer OK (Rust)"
```

For changes to the renderer, also compare against the CPU oracle, for example
`target/release/almondbread verify --center re,im --scale s --size 800x500 --iterations N`,
and `make regress`. CI runs the Rust checks on Linux and builds the app on macOS; it cannot
run the GPU checks, so please run them locally.
