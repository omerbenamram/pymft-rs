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
    """

    def __new__(cls, path_or_file_like: str | BinaryIO) -> Self: ...
    def number_of_entries(self) -> int:
        """Returns the total number of entries in the MFT."""

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

    A parser can only be iterated once.
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
        | PyMftAttributeX80
        | PyMftAttributeX90
        | PyMftAttributeNonResident
        | PyMftAttributeOther
        | None
    ):
        """The parsed resident content, the data runs of a non-resident attribute,
        or `None` if the attribute has no content."""

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
class PyMftAttributeNonResident:
    """The content of a non-resident attribute: the data runs that locate it on the volume."""

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
