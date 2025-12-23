# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.7.0]

- Align package version with `mft` core version (`0.7.0`).
- Migrate to newer PyO3 APIs (Bound / IntoPyObject) and restore compilation on modern toolchains.
- Modernize packaging (maturin `>=1.0`) and add GitHub Actions CI/release workflows (including Linux aarch64 wheels via zig cross-compilation).

## [0.6.1]

- Massive dependency update
