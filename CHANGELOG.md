# Changelog

All notable changes to this project will be documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Release checklist

Before tagging a new version:

1. Update crate versions in `Cargo.toml` and the `[workspace.package]` metadata.
2. Document the changes in the sections below (Added / Changed / Fixed / Removed).
3. Run `cargo fmt`, `cargo clippy --all-targets --all-features`, `cargo test --all --all-features`.
4. Regenerate `bench/report.html` for the public sample dataset and attach it to the release if it changed.
5. Verify docs (README, INSTALL, SECURITY, CODE_OF_CONDUCT) still reflect the intended release scope.
6. Create a signed git tag `vX.Y.Z`, push, and draft the GitHub release notes using the entries below.

## [Unreleased]
_Nothing yet_

## [0.2.0] - 2026-02-02
### Added
- Published [`three-dcf-core`](https://crates.io/crates/three-dcf-core) with crate metadata, README, examples, index/prelude modules, and bundled proto files.
- README instructions for consuming the core as a Rust library alongside CLI usage.

### Changed
- All workspace crates (doc2dataset, CLI, service, ffi targets) now depend on the published core crate name and import JSONL helpers from `three_dcf_core::index`.
- Workspace layout documentation highlights the publishable core crate.

### Removed
- Deprecated `crates/index` package; its types are merged into `three_dcf_core::index`.

## [0.1.0] - 2024-01-01
### Added
- Initial release with `three_dcf_core`, `three_dcf_cli`, `.3dcf` container format, CLI subcommands, bench harness, and plotting scripts.
