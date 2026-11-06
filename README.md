<div align="center">
  <!-- Downloads -->
  <a href="https://pypi.org/project/mft/">
    <img src="https://pepy.tech/badge/mft"
      alt="Download" />
  </a>
</div>


# pymft-rs

Python bindings for `https://github.com/omerbenamram/mft/`.

## Installation

Available on PyPi - https://pypi.org/project/mft/.

To install from PyPi - `pip install mft`

### Wheels
Wheels are built using the `abi3` tag, so a single wheel works for CPython 3.10 and newer.
Free-threaded CPython 3.14 (`python3.14t`) gets its own `cp314t` wheel; importing `mft` there
keeps the GIL disabled, and parsers can be created on one thread and consumed on another.

Supported platforms are:
  - Linux x86_64 and aarch64 (manylinux and musllinux; aarch64 and musl are cross-compiled with zig)
  - macOS universal2 (x86_64 + arm64)
  - Windows x86_64

The package ships type stubs (`mft.pyi`), checked against the compiled module in CI with `mypy.stubtest`.

### Installation from sources
Installation is possible for other platforms by installing from sources.

This requires a Rust compiler and a recent enough Pip.

Run `pip install -e .`


## Usage

A parser can be iterated more than once, and `parser.get_entry(5)` reads a single entry by number.

Iterating raises the `RuntimeError` or `OSError` of an entry or attribute that fails to parse, after stepping past it.
Pass `skip_errors=True` to skip them instead: each one skipped is reported as a `RuntimeWarning`, which
[`warnings` filters](https://docs.python.org/3/library/warnings.html#the-warnings-filter) can count, log or turn back into errors.
Failing to read the file raises either way, and so does reading an `attribute_content` that fails to parse.

```python
from mft import PyMftParser, PyMftAttributeX10, PyMftAttributeX30

def main():
    parser = PyMftParser("/Users/omerba/Workspace/pymft-rs/samples/MFT", skip_errors=True)
    for entry in parser.entries():
        print(f'Entry ID: {entry.entry_id}')

        for attribute in entry.attributes():
            resident_content = attribute.attribute_content
            if isinstance(resident_content, PyMftAttributeX10):
                print(f'Found an X10 attribute')
                print(f'Modified: {resident_content.modified}')
            if isinstance(resident_content, PyMftAttributeX30):
                print(f'Found an X30 attribute')
                print(f'Modified: {resident_content.modified}')
                print(f'Name: {resident_content.name}')

        print('--------------------------------')
```

Will print:

```
Entry ID: 0
Found an X10 attribute
Modified: 2007-06-30 12:50:52.252395+00:00
Found an X30 attribute
Modified: 2007-06-30 12:50:52.252395+00:00
Name: $MFT
--------------------------------
Entry ID: 1
Found an X10 attribute
Modified: 2007-06-30 12:50:52.252395+00:00
Found an X30 attribute
Modified: 2007-06-30 12:50:52.252395+00:00
Name: $MFTMirr
--------------------------------
.....
```

### Bodyfile and data streams

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
