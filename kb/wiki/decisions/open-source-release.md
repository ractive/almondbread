---
title: "Open-source release: licence, distribution, CI"
type: decision
date: 2026-10-17
status: accepted
tags: [distribution, licence, release, ci, macos]
---

# Open-source release: licence, distribution, CI

## Context

The user decided to publish almondbread as open source on GitHub, as the public repository
`github.com/ractive/almondbread`, with a free signed build for the Mac now and a Mac/iOS
App Store version later.
The release pipeline is modelled on the sibling project saturnus (its `desktop.yml`,
`ci.yml` and Homebrew cask templates).

## Decisions

- **Licence: MIT**, copyright James Bergamin, with the AI-generated-code paragraph and
  `AI_NOTICE` exactly as in saturnus. The Cargo workspace licence is `MIT` (it said
  `MIT OR Apache-2.0` before). Third-party material is listed in `THIRD_PARTY_NOTICES.md`:
  the Inter font (OFL 1.1, its text ships in the app bundle as `Inter-OFL.txt`), the
  colour maps of `palettes/derived.json` (CC0, MIT, Apache-2.0, BSD-style, CC BY 4.0, each
  with attribution), the crate licences. `deny.toml` (cargo-deny) bans everything but MIT,
  Apache-2.0, Zlib and Unicode-3.0, with MPL-2.0 scoped to the uniffi crates; GPL/AGPL is a
  blocker.
- **Code provenance:** algorithms and constants from Kalles Fraktaler, Fraktaler 3, Imagina
  (all AGPL) and FractalShark are used as published ideas and cited facts; no code is copied
  (audit 2026-10-17: `la.rs`, `bla.rs`, `la_gpu.rs` and the LA kernels are an independent
  formulation). See the code-licence rule in `kb/CLAUDE.md`.
- **Distribution now:** a Developer ID signed, notarised and stapled `.dmg` attached to a
  GitHub release, and the `almondbread` cask in the user's tap `ractive/homebrew-tap`
  (`brew install --cask ractive/tap/almondbread`). Apple silicon only, macOS 14+.
- **Later:** a paid Mac/iOS App Store version (one bundle ID `ch.ractive.almondbread` for
  Universal Purchase). Not in scope yet.
- **Donations:** Buy Me a Coffee and/or GitHub Sponsors were considered; dropped for the first
  release (no `FUNDING.yml`, no support link in the app). If added later, the link must be
  absent from App Store builds (Apple's rules), for example behind an Info.plist key that only
  the GitHub build sets.

## Pipeline

- `.github/workflows/ci.yml` on every push to main and every PR: on Linux rustfmt, clippy
  `-D warnings`, `cargo test --release` and cargo-deny for every crate but `almondbread-ffi`
  (Apple-only: it hosts the Metal renderer); math and core checked on Windows; `hyalo lint`;
  on macOS a clean-checkout build (xcodegen, `make rust` with the three Apple targets,
  UniFFI bindings, `make build`, `make app`, unsigned `.dmg` as an artifact).
  **No GPU tests in CI**: hosted macOS runners are VMs without a representative Metal
  device, so `make selftest`, `bench`, `verify` and `interaction-test` stay local.
- `.github/workflows/release.yml` on a `v*` tag: version from the tag
  (`CFBundleShortVersionString`, the run number as build number), sign, notarise and staple
  through `scripts/make-dmg.sh`, then a **draft** release with the `.dmg` and
  `almondbread-SHA256SUMS`. A manual run builds artifacts only. Unsigned builds are never
  attached to a release.
- Signing uses saturnus's six secret names, in the `release` environment:
  `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`,
  `APPLE_API_ISSUER`, `APPLE_API_KEY_ID`, `APPLE_API_KEY`. The certificate goes into a
  temporary keychain with Apple's Developer ID G2 intermediate (pinned SHA-256), deleted
  after the build.
- `.github/workflows/cask.yml` when a release is published (or by hand): checks the `.dmg`
  passes Gatekeeper and holds the right bundle ID and version, renders
  `packaging/homebrew/almondbread.rb` and pushes it to the tap with `HOMEBREW_TAP_TOKEN`.
- Locally: `make dmg VERSION=x.y.z [SIGN_IDENTITY=...] [NOTARY_PROFILE=...]`; it signs only
  with an identity from the keychain and notarises only with an existing notarytool keychain
  profile. No credential is stored in the repository.

## Open points (2026-10-17)

- Resolved 2026-10-17: the business pages (market research, Steam analysis, pricing,
  gamification, distribution) moved out of the repository into private notes before the
  first push.
- Source pages point at `raw/web/...` files that are not committed (copyrighted); the
  manifest says how to fetch them.
