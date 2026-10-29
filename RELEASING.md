# Releasing

This project is published to PyPI as `mft` using `maturin` + GitHub Actions.

## One-time setup

- Create a PyPI token and add it as a GitHub Actions secret named `PYPI_API_TOKEN`.

## Cut a release

1. Pick the version. While on 0.x, a breaking change or a new feature bumps the minor version
   and fixes alone bump the patch version.
2. Open a `chore(release): X.Y.Z` PR that bumps `version` in `Cargo.toml`, commits the refreshed
   `Cargo.lock` (any `cargo build` updates it; CI builds with `--locked`), and renames
   `## [Unreleased]` in `CHANGELOG.md` to `## [X.Y.Z] - YYYY-MM-DD`. If it needs a new `mft`
   crate, release that first and point the dependency at the crates.io version.
3. Merge it.

The merge runs `.github/workflows/auto-tag.yml`. It sees a `Cargo.toml` version with no tag,
pushes the `vX.Y.Z` tag on the merge commit, and starts `.github/workflows/ci.yml` on that tag
with `gh workflow run --ref vX.Y.Z` (a tag pushed with `GITHUB_TOKEN` doesn't trigger workflows,
but a dispatch does). Once
every wheel job (abi3 for CPython 3.10+, and `cp314t` for free-threaded 3.14, on Linux glibc/musl
x86_64 + aarch64, macOS universal2 and Windows) and the sdist build pass, its `release` job
publishes them to PyPI with `uv publish`.

Pushing a `vX.Y.Z` tag by hand still runs `ci.yml` directly; auto-tag then skips that version
because the tag exists. To retry a failed release, re-run the failed jobs of its run.
