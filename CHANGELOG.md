# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `PyMftParser.get_entry(entry_id)` reads one entry by number, with its `full_path`. It returns
  `None` for a record that is all zeroes (an entry `entries()` skips) and raises `IndexError`
  for a negative number or one past the last entry.
- `$VOLUME_NAME`, `$VOLUME_INFORMATION` and `$REPARSE_POINT` have their own content classes:
  `PyMftAttributeX60` (`name`), `PyMftAttributeX70` (`major_version`, `minor_version`, `flags`)
  and `PyMftAttributeXC0` (the tag and its name, a third-party tag's GUID, and the decoded
  target of symlinks, junctions and mount points, app execution aliases and WSL symlinks; other
  tags keep their tag-specific `data`).
- `PyMftAttribute.raw_value` is a resident attribute's value as stored in the record (`None`
  for non-resident attributes), whatever its content class.
- `PyMftEntry` has `lsn` (the $LogFile sequence number), `is_allocated`, `is_dir` and
  `valid_fixup`.
- Flags are also available as integers: `PyMftEntry.flags_value`,
  `PyMftAttribute.data_flags_value`, `PyMftAttributeX10.file_flags_value` and
  `PyMftAttributeX30.flags_value`.
- `PyMftAttributeNonResident` has the attribute's sizes (`allocated_length`, `file_size`,
  `valid_data_length`, `total_allocated`), the VCN range it covers and `unit_compression_size`.
- `PyMftAttributeX20Entry` has the extension record that holds each attribute
  (`segment_entry_id`, `segment_entry_sequence`).

### Changed

- **Breaking:** `attribute_content` of `$VOLUME_NAME`, `$VOLUME_INFORMATION` and
  `$REPARSE_POINT` is now `PyMftAttributeX60`, `PyMftAttributeX70` or `PyMftAttributeXC0`
  instead of `PyMftAttributeOther`. The raw value is in the new `PyMftAttribute.raw_value`.
  The `data` of an undecoded reparse point is the bytes after its 8-byte header (and a
  third-party tag's GUID). A value too short to parse is still `PyMftAttributeOther`.
- A `PyMftParser` can be iterated any number of times, and `number_of_entries()` works after
  iterating. Before, a second `entries()`, `entries_json()` or `entries_csv()` raised
  `RuntimeError: PyMftParser can only be used once`. Iterators share the parser, including its
  cache of directory paths, and each has its own position. Threads can share a parser too.

## [0.8.0] - 2026-10-27

### Added

- Non-resident attributes expose their data runs: `attribute_content` returns a
  `PyMftAttributeNonResident` whose `data_runs()` lists `PyMftDataRun` objects (`lcn_offset`,
  `lcn_length`, `is_sparse`). Ported from #12 by @uckelman-sf.
- `PyMftAttributeX90` (`$INDEX_ROOT`) exposes `relative_offset_to_index_node`,
  `index_node_length`, `index_node_allocation_length`, `is_large_index`, and `index_entries()`,
  a list of `PyMftIndexEntry` objects with the referenced entry and its `$FILE_NAME` copy
  (`fname_info`). Ported from #12.

### Changed

- **Breaking:** `attribute_content` of a non-resident attribute is now a
  `PyMftAttributeNonResident` instead of `None`.
- **Breaking:** `PyMftEntry.full_path` uses `\` separators on every platform (Linux and macOS
  builds used `/`), replacing #13 by @uckelman-sf.
- **Breaking:** require Python 3.10 or newer (the abi3 floor is now `abi3-py310`, matching `evtx`).
- Build on mft 0.8.0. It parses attributes without copying the record, which makes
  `entries_json()`/`entries_csv()` and attribute access faster.
- `entries_json()` and `entries_csv()` follow mft 0.8.0's output: timestamps keep NTFS's 100ns
  resolution (7 fractional digits), `$VOLUME_NAME`/`$VOLUME_INFORMATION`/`$REPARSE_POINT` are
  parsed in JSON instead of hex, and CSV paths use `\` and gain `StandardInfoLastMftModified` and
  `FileNameLastMftModified` columns at the end. See mft's CHANGELOG for the full list.
- Update to PyO3 0.30.1 and pyo3-file 0.18.0, and move the crate to the Rust 2024 edition.
- Build free-threaded CPython 3.14 (`cp314t`) wheels alongside the abi3 wheels, and add musllinux
  wheels for x86_64 and aarch64. The module declares `gil_used = false`.
- Parser, entry and iterator classes are no longer `unsendable`, so they can be passed between
  threads instead of raising when touched from a thread other than the one that created them.
- Classes now report `mft` as their `__module__` (was `builtins`).
- `PyMftEntry.attributes()` parses only attribute headers; content is parsed when `attribute_content` is read. An attribute whose content fails to parse is now listed, and the error is raised from `attribute_content` (it used to replace the whole attribute with an exception).
- `PyMftAttributeX20Entry` and `PyMftX20EntriesIter` are now exported from the module.
- Replace the `Any`-typed stub with precise types, verified in CI with `mypy.stubtest`.
- Fold wheel building and PyPI publishing into a single CI workflow (publishes on `v*` tags with
  `uv publish`), and verify each wheel's ABI tag before upload.

### Fixed

- Fix `PyMftEntry.hard_link_count` and `base_entry_sequence`, which were always 0.
- Format flags as `A | B` instead of bitflags' debug format `TypeName(A | B)`.
- `data_runs()` gives the absolute LCN of a run that follows a sparse run. With mft 0.7.0 it was
  relative to the sparse run's (zero) offset.
- `PyMftAttributeX30.logical_size` and `physical_size` are no longer swapped.
- `$STANDARD_INFORMATION` attributes from NTFS 1.2 (72 bytes) report 0 for `owner_id`,
  `security_id`, `quota` and `usn` instead of reading past the attribute.
- Named resident attributes (such as `$BITMAP:$SDH`) return their own value as `data`. With
  mft 0.7.0 it was read from the wrong offset.

## [0.7.0]

- Align package version with `mft` core version (`0.7.0`).
- Migrate to newer PyO3 APIs (Bound / IntoPyObject) and restore compilation on modern toolchains.
- Modernize packaging (maturin `>=1.0`) and add GitHub Actions CI/release workflows (including Linux aarch64 wheels via zig cross-compilation).

## [0.6.1]

- Massive dependency update
