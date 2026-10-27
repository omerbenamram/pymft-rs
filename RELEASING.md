# Releasing

This project is published to PyPI as `mft` using `maturin` + GitHub Actions.

## One-time setup

- Create a PyPI token and add it as a GitHub Actions secret named `PYPI_API_TOKEN`.

## Cut a release

- Bump the version in `Cargo.toml` and commit the refreshed `Cargo.lock` (any `cargo build` updates it; CI builds with `--locked`).
- Add release notes to `CHANGELOG.md`.
- Tag and push:

```bash
git tag -a vX.Y.Z -m "vX.Y.Z"
git push origin vX.Y.Z
```

Pushing the tag runs `.github/workflows/ci.yml`. Once every wheel job (abi3 for CPython 3.10+, and
`cp314t` for free-threaded 3.14, on Linux glibc/musl x86_64 + aarch64, macOS universal2 and Windows)
and the sdist build pass, its `release` job publishes them to PyPI with `uv publish`.
