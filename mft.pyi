"""Python bindings for https://github.com/omerbenamram/mft."""

import datetime
from typing import BinaryIO, TypeAlias, final

from typing_extensions import Self

# Iterators yield the error object instead of raising it, so iteration can continue
# past a broken entry or attribute: `OSError` for I/O failures, `RuntimeError` otherwise.
_ParseError: TypeAlias = RuntimeError | OSError

@final
class PyMftParser:
    """Returns an instance of the parser.

    Works on both a path (string), or a file-like object opened in binary mode.

    A parser can be iterated any number of times, and `get_entry` reads one entry by
    number, including while an iteration is in progress. Threads can share a parser.
    """

    def __new__(cls, path_or_file_like: str | BinaryIO) -> Self: ...
    def number_of_entries(self) -> int:
        """Returns the total number of entries in the MFT."""

    def get_entry(self, entry_id: int) -> PyMftEntry | None:
        """Returns the entry with this number, or `None` if its record is all zeroes
        (an entry `entries()` skips). Raises `IndexError` for a negative number or one
        past the last entry."""

    def entries(self) -> PyMftEntriesIterator:
        """Returns an iterator that yields the mft entries as python objects."""

    def entries_json(self) -> PyMftEntriesIterator:
        """Returns an iterator that yields mft entries as JSON strings."""

    def entries_csv(self) -> PyMftEntriesIterator:
        """Returns an iterator that yields mft entries as CSV lines (bytes)."""

    def __iter__(self) -> PyMftEntriesIterator: ...
    def __next__(self) -> PyMftEntry | _ParseError: ...

@final
class PyMftEntriesIterator:
    """Yields `PyMftEntry` objects from `entries()`, `str` from `entries_json()`,
    `bytes` from `entries_csv()`, or a parse error in place of a broken entry.

    Each iterator has its own position.
    """

    def __iter__(self) -> Self: ...
    def __next__(self) -> PyMftEntry | str | bytes | _ParseError: ...

@final
class PyMftEntry:
    entry_id: int
    sequence: int
    base_entry_id: int
    base_entry_sequence: int
    hard_link_count: int
    flags: str
    flags_value: int
    """`flags` as an integer."""
    is_allocated: bool
    is_dir: bool
    lsn: int
    """The $LogFile sequence number (LSN) of the last logged change to this record."""
    valid_fixup: bool | None
    """Whether the update sequence (fixup) values matched at the end of every sector.
    `None` when the record has no update sequence array."""
    used_entry_size: int
    total_entry_size: int
    full_path: str
    file_size: int
    def attributes(self) -> PyMftAttributesIter: ...

@final
class PyMftAttributesIter:
    def __iter__(self) -> Self: ...
    def __next__(self) -> PyMftAttribute | _ParseError: ...

@final
class PyMftAttribute:
    type_code: int
    """Numeric value of the attribute type."""
    type_name: str
    """Name of the attribute type."""
    name: str
    """Attribute name (can be empty)."""
    data_flags: str
    data_flags_value: int
    """`data_flags` as an integer."""
    is_resident: bool
    data_size: int
    @property
    def attribute_content(
        self,
    ) -> (
        PyMftAttributeX10
        | PyMftAttributeX20
        | PyMftAttributeX30
        | PyMftAttributeX40
        | PyMftAttributeX60
        | PyMftAttributeX70
        | PyMftAttributeX80
        | PyMftAttributeX90
        | PyMftAttributeXC0
        | PyMftAttributeNonResident
        | PyMftAttributeOther
    ):
        """The parsed resident content, or the data runs of a non-resident attribute.
        Attributes without a class of their own give their raw value as `PyMftAttributeOther`."""

@final
class PyMftAttributeX10:
    """$STANDARD_INFORMATION"""

    max_version: int
    version: int
    class_id: int
    owner_id: int
    security_id: int
    quota: int
    usn: int
    @property
    def created(self) -> datetime.datetime: ...
    @property
    def modified(self) -> datetime.datetime: ...
    @property
    def mft_modified(self) -> datetime.datetime: ...
    @property
    def accessed(self) -> datetime.datetime: ...
    @property
    def file_flags(self) -> str: ...
    @property
    def file_flags_value(self) -> int:
        """`file_flags` as an integer."""

@final
class PyMftAttributeX20:
    """$ATTRIBUTE_LIST"""

    def entries(self) -> PyMftX20EntriesIter: ...

@final
class PyMftX20EntriesIter:
    def __iter__(self) -> Self: ...
    def __next__(self) -> PyMftAttributeX20Entry | _ParseError: ...

@final
class PyMftAttributeX20Entry:
    attribute_type: int
    lowest_vcn: int
    name: str
    segment_entry_id: int
    """The entry that holds the attribute: this entry, or one of its extension records."""
    segment_entry_sequence: int

@final
class PyMftAttributeX30:
    """$FILE_NAME"""

    parent_entry_id: int
    parent_entry_sequence: int
    logical_size: int
    physical_size: int
    reparse_value: int
    namespace: str
    name: str
    @property
    def created(self) -> datetime.datetime: ...
    @property
    def modified(self) -> datetime.datetime: ...
    @property
    def mft_modified(self) -> datetime.datetime: ...
    @property
    def accessed(self) -> datetime.datetime: ...
    @property
    def flags(self) -> str: ...
    @property
    def flags_value(self) -> int:
        """`flags` as an integer."""

@final
class PyMftAttributeX40:
    """$OBJECT_ID"""

    object_id: str
    """Unique Id assigned to file."""
    birth_volume_id: str
    """Volume where file was created."""
    birth_object_id: str
    """Original Object Id of file."""
    domain_id: str
    """Domain in which object was created."""

@final
class PyMftAttributeX60:
    """$VOLUME_NAME: the volume label."""

    name: str

@final
class PyMftAttributeX70:
    """$VOLUME_INFORMATION: the NTFS version and volume state flags."""

    major_version: int
    minor_version: int
    flags: str
    flags_value: int
    """`flags` as an integer."""

@final
class PyMftAttributeX80:
    """$DATA (resident)"""

    @property
    def data(self) -> bytes: ...

@final
class PyMftAttributeX90:
    """$INDEX_ROOT"""

    attribute_type: int
    collation_rule: int
    """Collation rule used to sort the index entries."""
    index_entry_size: int
    index_entry_number_of_cluster_blocks: int
    relative_offset_to_index_node: int
    index_node_length: int
    index_node_allocation_length: int
    is_large_index: bool
    """Whether the index continues in `$INDEX_ALLOCATION` (only the entries in the
    index root are available here)."""
    def index_entries(self) -> list[PyMftIndexEntry]:
        """The index entries stored in the index root itself."""

@final
class PyMftIndexEntry:
    """An entry of a `$FILE_NAME` index (`$I30`), pointing at the file it names."""

    mft_reference_entry_id: int
    mft_reference_entry_sequence: int
    index_record_length: int
    attr_fname_length: int
    flags: str
    fname_info: PyMftAttributeX30
    """The copy of the file's `$FILE_NAME` attribute kept in the index."""

@final
class PyMftAttributeXC0:
    """$REPARSE_POINT. `reparse_type` says which of the optional fields are set:

    - `SymbolicLink`: `substitute_name`, `print_name` and `is_relative`
    - `MountPoint` (a junction or volume mount point): `substitute_name` and `print_name`
    - `AppExecLink`: `package_id`, `app_user_model_id` and `target_path`
    - `LxSymlink` (a WSL symlink): `target`
    - `Other` (any other tag, or data that doesn't match its tag): `data`
    """

    tag: int
    """The reparse tag, which says which file system filter owns the data."""
    tag_name: str | None
    """The name of a documented tag, such as `SYMLINK`, without the `IO_REPARSE_TAG_`
    prefix."""
    guid: str | None
    """The owning filter's GUID, for third-party tags."""
    reparse_type: str
    substitute_name: str | None
    print_name: str | None
    is_relative: bool | None
    package_id: str | None
    app_user_model_id: str | None
    target_path: str | None
    target: str | None
    @property
    def data(self) -> bytes | None:
        """The raw data of an `Other` reparse point."""

@final
class PyMftAttributeNonResident:
    """The content of a non-resident attribute: its sizes, and the data runs that locate it
    on the volume. The sizes are only valid in the attribute record whose `vcn_first` is 0."""

    vcn_first: int
    """The first virtual cluster number (VCN) this attribute record covers."""
    vcn_last: int
    """The last VCN this attribute record covers."""
    allocated_length: int
    """Allocated size in bytes, a multiple of the cluster size."""
    file_size: int
    """Real size in bytes."""
    valid_data_length: int
    """Initialized size in bytes. Reads past it return zeroes."""
    unit_compression_size: int
    """Compression unit size, as a power of two in clusters (0 if not compressed)."""
    total_allocated: int | None
    """Bytes actually allocated on disk. Only stored for compressed attributes."""
    def data_runs(self) -> list[PyMftDataRun]: ...

@final
class PyMftDataRun:
    """A contiguous run of clusters. `lcn_offset` is the absolute cluster number where the run
    starts, and is 0 for sparse runs, which have no clusters on disk."""

    lcn_offset: int
    lcn_length: int
    is_sparse: bool

@final
class PyMftAttributeOther:
    """An attribute whose content is not parsed by the Rust library."""

    @property
    def data(self) -> bytes: ...
