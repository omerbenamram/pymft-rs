"""Python bindings for https://github.com/omerbenamram/mft."""

import datetime
from typing import BinaryIO, NoReturn, final

from typing_extensions import Self

@final
class PyMftParser:
    """Returns an instance of the parser.

    Works on both a path (string), or a file-like object opened in binary mode.

    A parser can be iterated any number of times, and `get_entry` reads one entry by
    number, including while an iteration is in progress. Threads can share a parser.

    Iterating over entries or attributes raises `RuntimeError` or `OSError` for one that
    fails to parse, after stepping past it, so `next()` can go on. With `skip_errors=True`
    it is skipped instead, with a `RuntimeWarning` that `warnings` filters can count, log
    or turn back into an error. Failing to read the file raises either way.
    """

    def __new__(
        cls, path_or_file_like: str | BinaryIO, *, skip_errors: bool = False
    ) -> Self: ...
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
    def __next__(self) -> NoReturn:
        """Raises `NotImplementedError`: iterate over `entries()` instead."""

@final
class PyMftEntriesIterator:
    """Yields `PyMftEntry` objects from `entries()`, `str` from `entries_json()` and
    `bytes` from `entries_csv()`.

    Each iterator has its own position.
    """

    def __iter__(self) -> Self: ...
    def __next__(self) -> PyMftEntry | str | bytes: ...

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
    `None` when the record's signature isn't `FILE` (such as a `BAAD` record), so the fixups
    weren't checked."""
    used_entry_size: int
    total_entry_size: int
    full_path: str
    file_size: int
    def attributes(self) -> PyMftAttributesIter: ...

@final
class PyMftAttributesIter:
    def __iter__(self) -> Self: ...
    def __next__(self) -> PyMftAttribute: ...

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
    def raw_value(self) -> bytes | None:
        """The value of a resident attribute as stored in the record, or `None` for a
        non-resident one. Parsed classes don't keep every byte (reserved fields, slack after
        names), so this is how to get them."""
    @property
    def attribute_content(
        self,
    ) -> (
        PyMftAttributeX10
        | PyMftAttributeX20
        | PyMftAttributeX30
        | PyMftAttributeX40
        | PyMftAttributeX50
        | PyMftAttributeX60
        | PyMftAttributeX70
        | PyMftAttributeX80
        | PyMftAttributeX90
        | PyMftAttributeXC0
        | PyMftAttributeXD0
        | PyMftAttributeXE0
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
    def __next__(self) -> PyMftAttributeX20Entry: ...

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
class PyMftAttributeX50:
    """$SECURITY_DESCRIPTOR: a resident security descriptor. Since NTFS 3.0 most files only
    have a security id in `$STANDARD_INFORMATION`, and the descriptor is in `$Secure`."""

    revision: int
    control: str
    control_value: int
    """`control` as an integer."""
    owner: str | None
    """The owner, as a SID string (`S-1-5-18`), or `None` when the descriptor has none."""
    group: str | None
    sacl: PyMftAcl | None
    """Only set when `control` has `SE_SACL_PRESENT`."""
    dacl: PyMftAcl | None
    """Only set when `control` has `SE_DACL_PRESENT`. A present flag without a DACL is a
    NULL DACL, which grants everyone full access."""

@final
class PyMftAcl:
    """An access control list."""

    revision: int
    def entries(self) -> list[PyMftAce]: ...

@final
class PyMftAce:
    """An access control entry."""

    ace_type: int
    ace_type_name: str | None
    """The name of a documented type (for example `ACCESS_ALLOWED`), without the
    `_ACE_TYPE` suffix."""
    flags: str
    flags_value: int
    """`flags` as an integer."""
    access_mask: int | None
    """`None` for types whose layout isn't decoded."""
    object_type: str | None
    """The object type of an object ACE, when its flags say it has one."""
    inherited_object_type: str | None
    """The inherited object type of an object ACE, when its flags say it has one."""
    sid: str | None
    @property
    def data(self) -> bytes:
        """The bytes after the decoded fields (for example a callback ACE's application
        data), or the whole body for types whose layout isn't decoded."""

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
        """The index entries stored in the index root itself, for a `$FILE_NAME` index
        (`$I30`)."""
    def view_entries(self) -> list[PyMftViewIndexEntry]:
        """The index entries stored in the index root itself, for any other index (a view
        index such as `$Secure:$SII` or `$Extend\\$Quota:$Q`)."""

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
class PyMftViewIndexEntry:
    """An entry of a view index: one of the indexes NTFS keeps in its metadata files,
    recognized from the index's collation rule and the sizes of the entry's key and data.
    `value_type` says which of the optional fields are set:

    - `SecurityDescriptor` (`$Secure:$SDH` or `$Secure:$SII`): `hash`, `security_id`,
      `sds_offset` and `sds_length`
    - `ObjectId` (`$Extend\\$ObjId:$O`): `object_id`, `mft_reference_entry_id`,
      `mft_reference_entry_sequence`, `birth_volume_id`, `birth_object_id` and `domain_id`
    - `QuotaOwner` (`$Extend\\$Quota:$O`): `sid` and `owner_id`
    - `Quota` (`$Extend\\$Quota:$Q`): `owner_id`, `version`, `quota_flags`, `bytes_used`,
      `change_time`, `warning_limit` and `hard_limit`, and `exceeded_time` and `sid` when
      the entry has them
    - `Reparse` (`$Extend\\$Reparse:$R`): `tag`, `tag_name`, `mft_reference_entry_id` and
      `mft_reference_entry_sequence`
    - `Other` (any other index): `key` and `data`
    """

    flags: str
    flags_value: int
    """`flags` as an integer."""
    value_type: str
    hash: int | None
    security_id: int | None
    sds_offset: int | None
    sds_length: int | None
    object_id: str | None
    mft_reference_entry_id: int | None
    mft_reference_entry_sequence: int | None
    birth_volume_id: str | None
    birth_object_id: str | None
    domain_id: str | None
    sid: str | None
    owner_id: int | None
    version: int | None
    quota_flags: int | None
    bytes_used: int | None
    warning_limit: int | None
    """-1 means no limit."""
    hard_limit: int | None
    """-1 means no limit."""
    tag: int | None
    tag_name: str | None
    """The name of a documented tag, as in `PyMftAttributeXC0.tag_name`."""
    @property
    def change_time(self) -> datetime.datetime | None:
        """When the quota entry last changed."""
    @property
    def exceeded_time(self) -> datetime.datetime | None:
        """When the owner went over their warning limit, if they did."""
    @property
    def key(self) -> bytes | None:
        """The key of an `Other` entry."""
    @property
    def data(self) -> bytes | None:
        """The data of an `Other` entry."""

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
        """The tag-specific data of an `Other` reparse point: the bytes after the reparse
        header (and the GUID of a third-party tag)."""

@final
class PyMftAttributeXD0:
    """$EA_INFORMATION: the sizes of the file's extended attributes in `$EA`."""

    packed_ea_size: int
    """The size of the extended attributes in packed form: each one's flags, lengths, name
    and value, without the next entry offsets and padding that `$EA` stores."""
    need_ea_count: int
    """How many extended attributes have the `NEED_EA` flag."""
    unpacked_ea_size: int
    """The size of the extended attributes as `$EA` stores them, which is the size of its
    value."""

@final
class PyMftAttributeXE0:
    """$EA: the file's extended attributes. Windows keeps its own metadata here too, such as
    WSL's Linux owners and modes (`$LXUID`, `$LXGID`, `$LXMOD`, `$LXDEV`) and code
    integrity's `$CI.CATALOGHINT`."""

    def entries(self) -> list[PyMftEaEntry]: ...

@final
class PyMftEaEntry:
    """One extended attribute. `value_type` says which of the optional fields are set:

    - `LxInteger` (`$LXUID`, `$LXGID` or `$LXMOD`, a WSL file's Linux owner, group or
      mode): `value`
    - `LxDevice` (`$LXDEV`, the device numbers of a WSL device file): `major` and `minor`
    - `CatalogHint` (`$CI.CATALOGHINT`, the security catalog that signs the file):
      `catalog`
    - `Other` (any other value, or one that doesn't match its name's layout): `data`
    """

    flags: str
    flags_value: int
    """`flags` as an integer."""
    name: str
    value_type: str
    value: int | None
    major: int | None
    minor: int | None
    catalog: str | None
    @property
    def data(self) -> bytes | None:
        """The value of an `Other` entry."""

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
    """Bytes actually allocated on disk. Only read for compressed attributes, `None`
    otherwise."""
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
