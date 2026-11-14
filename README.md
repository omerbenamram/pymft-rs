# pymft-rs

[![PyPI](https://img.shields.io/pypi/v/mft.svg)](https://pypi.org/project/mft/)
[![Downloads](https://pepy.tech/badge/mft)](https://pypi.org/project/mft/)

Read an NTFS Master File Table (`$MFT`) from Python. These are the Python bindings for the Rust [`mft`](https://github.com/omerbenamram/mft) parser.

```
pip install mft
```

This example reads [`samples/MFT`](https://github.com/omerbenamram/pymft-rs/tree/master/samples) from this repository. Give it the path of your own `$MFT` instead.

```python
from mft import PyMftParser, PyMftAttributeX10, PyMftAttributeX30

parser = PyMftParser("samples/MFT")

# Deleted executables that are still in the table.
for entry in parser.entries():
    if not entry.is_allocated and entry.full_path.lower().endswith(".exe"):
        print(entry.entry_id, entry.file_size, entry.full_path)

# The timestamps and names of the one it found.
entry = parser.get_entry(12938)
for attribute in entry.attributes():
    content = attribute.attribute_content
    if isinstance(content, PyMftAttributeX10):    # $STANDARD_INFORMATION
        print("created ", content.created)
        print("modified", content.modified)
    elif isinstance(content, PyMftAttributeX30):  # $FILE_NAME
        print("name    ", content.name)
```

```
12938 0 [Orphaned]\winzip120[1].exe
created  2009-01-14 21:09:23.091300+00:00
modified 2009-01-14 21:09:36.410452+00:00
name     WINZIP~1.EXE
name     winzip120[1].exe
```

## What you get

- `parser.entries()` yields a `PyMftEntry` for each record: `entry_id`, `full_path`, `file_size`, `is_allocated`, `is_dir`, `flags` and more. It leaves out records that are all zeros, so it can yield fewer than `parser.number_of_entries()`. `parser.get_entry(5)` reads one record by number, and returns `None` for a record that is all zeros. A parser can be iterated more than once.
- `entry.attributes()` yields the record's attributes. `attribute.attribute_content` is a `PyMftAttributeX10` for `$STANDARD_INFORMATION`, a `PyMftAttributeX30` for `$FILE_NAME`, and so on. That holds for an attribute stored inside the record. One stored out on the disk, as the `$DATA` of most files is, is a `PyMftAttributeNonResident` whatever its type. An attribute type without a class of its own is a `PyMftAttributeOther` with the raw value. Timestamps are timezone-aware `datetime` objects in UTC.
- `parser.entries_csv()` and `parser.entries_json()` are for when you want every record and not Python objects. `entries_csv()` yields `bytes`, and joined together they are the output of `mft_dump -o csv`; the first item holds the header line and the first row. `entries_json()` yields one `str` per record. It is the record that `mft_dump -o jsonl` writes, without the `full_path` key that `mft_dump` adds.
- `entry.full_path` is `[UNKNOWN]` for a record without a `$FILE_NAME` attribute, where `entries_csv()` has an empty `FullPath`. A path that starts with `[Orphaned]` is that of a file whose parent folder is gone, and one that starts with `[Unknown]` has a parent record that is not a folder.

The package ships type stubs (`mft.pyi`), so an editor shows every class and attribute.

## Bodyfile and data streams

`entries_bodyfile()` yields Sleuth Kit bodyfile lines (the same as `mft_dump -o bodyfile` and `fls -m`), for timelining with `mactime`.
`data_streams(entry_id)` returns a file's `$DATA` streams, including alternate data streams and ones stored in extension records,
with the content of the resident ones.

```python
from mft import PyMftParser

parser = PyMftParser("samples/MFT")

with open("mft.body", "w", encoding="utf-8", newline="") as body:
    body.writelines(parser.entries_bodyfile())

for stream in parser.data_streams(12938):
    if stream.name == "Zone.Identifier" and stream.data is not None:
        print(stream.data.decode())  # [ZoneTransfer] ZoneId=3
```

So the deleted `winzip120[1].exe` above came from the internet zone.

## Errors and warnings

Real `$MFT`s have damaged records, so decide what should happen when the parser meets one.

- By default, iterating raises the `RuntimeError` or `OSError` of an entry or attribute that fails to parse, after stepping past it. A `for` loop ends there. If you catch the error and call `next()` again, the iterator goes on with the next entry.
- With `PyMftParser(path, skip_errors=True)` each such entry or attribute is skipped and reported as a `RuntimeWarning`. An entry with a skipped attribute is still returned, with its other attributes. [`warnings` filters](https://docs.python.org/3/library/warnings.html#the-warnings-filter) can count them, log them or turn them back into errors.
- Failing to read the file raises either way, and so does reading an `attribute_content` that fails to parse.
- What the parser notices but reads anyway, such as a record whose fixup values don't match, is reported as a `UserWarning`. With a filter that turns it into an error (`-W error`), the call that read the record raises it and the entry is not returned: `get_entry` raises every time, and an iterator steps past the entry, as it does for one that fails to parse.

## Paths and file objects

`PyMftParser` takes a path (`str`, `bytes` or `os.PathLike`, such as `pathlib.Path`) or a file-like object opened in binary mode. An object with `read` and `seek` is read as a file object, also when it is an `os.PathLike`.

A parser over a file object seeks before every read. Entries read in order are fetched 64 KiB at a time, so up to that much ahead of the entry asked for. On one thread this means the object can be used elsewhere between the parser's calls, by a second parser too.

It does not make one file object safe to use from two threads at once. A seek and its read are two calls, so two parsers over one file object (or a parser and your own reads) on different threads can interleave them, and a parser can then return the bytes of another record. Share one parser between the threads, or give each parser its own file object.

## Threads

Threads can share a parser, and an iterator too: each entry it yields goes to one of them. On free-threaded CPython 3.14, importing `mft` keeps the GIL disabled, and a parser can be created on one thread and used on another.

## Wheels

`pip install mft` gets a wheel, with nothing to compile, on:

- Linux x86_64 and aarch64 (manylinux and musllinux)
- macOS (universal2: x86_64 and arm64)
- Windows x86_64

One `abi3` wheel covers CPython 3.10 and newer. Free-threaded CPython 3.14 (`python3.14t`) has its own `cp314t` wheel.

On another platform, `pip install mft` builds the package from source, which needs a Rust compiler and a recent pip.
