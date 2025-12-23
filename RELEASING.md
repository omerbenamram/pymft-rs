# Releasing

This project is published to PyPI as `mft` using `maturin` + GitHub Actions.

## One-time setup

- Create a PyPI token and add it as a GitHub Actions secret named `PYPI_API_TOKEN`.

## Cut a release

- Bump the version in `Cargo.toml`.
- Add release notes to `CHANGELOG.md`.
- Tag and push:

```bash
git tag -a vX.Y.Z -m "vX.Y.Z"
git push --tags
```

Pushing the tag triggers `.github/workflows/release.yml`, which builds wheels/sdist and publishes them to PyPI.


