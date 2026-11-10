"""Python bindings for https://github.com/omerbenamram/mft."""

import datetime
import os
from typing import BinaryIO, final

from typing_extensions import Self

__version__: str

@final
class PyMftParser:
    """Returns an instance of the parser.

    Works on both a path (`str`, `bytes` or `os.PathLike`, such as `pathlib.Path`), or a
    file-like object opened in binary mode. Raises `TypeError` for anything else, a file
    opened in text mode included.

    A parser can be iterated any number of times, and `get_entry` reads one entry by
    number, including while an iteration is in progress. Threads can share a parser.

    A file-like object is read at the parser's own position: the parser seeks before every
    read, so the object can be used elsewhere between calls, by another parser too. Not from
    another thread at the same time, as a seek and its read are two calls.

    Iterating over entries or attributes raises `RuntimeError` or `OSError` for one that
    fails to parse, after stepping past it, so `next()` can go on. With `skip_errors=True`
    it is skipped instead, with a `RuntimeWarning` that `warnings` filters can count, log
    or turn back into an error. Failing to read the file raises either way.

    What the parser notices but reads anyway, such as a record whose fixup values don't
    match, is reported as a `UserWarning`. If a `warnings` filter turns it into an error,
    the call that read the record raises it.
    """

    def __new__(
        cls,
        path_or_file_like: str
        | bytes
        | os.PathLike[str]
        | os.PathLike[bytes]
        | BinaryIO,
        *,
        skip_errors: bool = False,
    ) -> Self: ...
    def number_of_entries(self) -> int:
        """Returns the total number of entries in the MFT."""

    def get_entry(self, entry_id: int) -> PyMftEntry | None:
        """Returns the entry with this number, or `None` if its record is all zeroes
        (an entry `entries()` skips). Raises `IndexError` for a negative number or one
        past the last entry, however large."""

    def entries(self) -> PyMftEntriesIterator:
        """Returns an iterator that yields the mft entries as python objects."""

    def entries_json(self) -> PyMftEntriesIterator:
        """Returns an iterator that yields mft entries as JSON strings."""

    def entries_csv(self) -> PyMftEntriesIterator:
        """Returns an iterator that yields mft entries as CSV lines (bytes)."""

    def __iter__(self) -> PyMftEntriesIterator: ...

@final
class PyMftEntriesIterator:
    """Yields `PyMftEntry` objects from `entries()`, `str` from `entries_json()` and
    `bytes` from `entries_csv()`.

    Each iterator has its own position. Threads can share an iterator: each entry goes to
    one of them.
    """

    def __iter__(self) -> Self: ...
    def __next__(self) -> PyMftEntry | str | bytes: ...

@final
class PyMftEntry:
    """An MFT record. Its attributes are read-only.

    Entries are equal (and hash alike) when they have the same number, record bytes and
    path, whichever parser or call they came from.
    """

    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...
    @property
    def entry_id(self) -> int: ...
    @property
    def sequence(self) -> int: ...
    @property
    def base_entry_id(self) -> int: ...
    @property
    def base_entry_sequence(self) -> int: ...
    @property
    def hard_link_count(self) -> int: ...
    @property
    def flags(self) -> str: ...
    @property
    def flags_value(self) -> int:
        """`flags` as an integer."""
    @property
    def is_allocated(self) -> bool: ...
    @property
    def is_dir(self) -> bool: ...
    @property
    def lsn(self) -> int:
        """The $LogFile sequence number (LSN) of the last logged change to this record."""
    @property
    def valid_fixup(self) -> bool | None:
        """Whether the update sequence (fixup) values matched at the end of every sector.
        `None` when the record's signature isn't `FILE` (such as a `BAAD` record), so the fixups
        weren't checked."""
    @property
    def used_entry_size(self) -> int: ...
    @property
    def total_entry_size(self) -> int: ...
    @property
    def full_path(self) -> str:
        """The path from the volume's root, with `\\` between names. `[UNKNOWN]` for a base
        record without a `$FILE_NAME` (where `entries_csv()` has an empty `FullPath`). A path
        can also start with a name that the mft crate gives: `[Orphaned]` when the parent
        directory is gone, `[Unknown]` when the parent's record isn't a directory."""
    @property
    def file_size(self) -> int:
        """The size of the unnamed `$DATA` stream, or of a named one if there is none, including
        when an extension record holds it. 0 without a `$DATA` attribute. The same as `FileSize`
        in `entries_csv()`."""
    def attributes(self) -> PyMftAttributesIter: ...

@final
class PyMftAttributesIter:
    def __iter__(self) -> Self: ...
    def __next__(self) -> PyMftAttribute: ...

@final
class PyMftAttribute:
    @property
    def type_code(self) -> int:
        """Numeric value of the attribute type."""
    @property
    def type_name(self) -> str:
        """Name of the attribute type."""
    @property
    def name(self) -> str:
        """Attribute name (can be empty)."""
    @property
    def data_flags(self) -> str: ...
    @property
    def data_flags_value(self) -> int:
        """`data_flags` as an integer."""
    @property
    def is_resident(self) -> bool: ...
    @property
    def data_size(self) -> int:
        """The length of the attribute's record, header included, which `entries_json()` calls
        `record_length`. A resident attribute's value is `raw_value`."""
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
    """$STANDARD_INFORMATION: the file's timestamps and flags and, since NTFS 3.0, its quota
    owner, quota charge, security id and update sequence number. The older layout lacks those
    four, which read as 0. Files can keep that layout on a newer volume. To tell the layouts
    apart, check the length of the `raw_value` of the `PyMftAttribute` this came from: 48 bytes
    in the older layout, at least 72 in the newer one. Not its `data_size`, which is at least 72
    in both."""

    @property
    def max_version(self) -> int: ...
    @property
    def version(self) -> int: ...
    @property
    def class_id(self) -> int: ...
    @property
    def owner_id(self) -> int:
        """The key of the file's owner in `$Extend\\$Quota`. 0 if quotas have never been enabled
        on the volume, or in the older layout."""
    @property
    def security_id(self) -> int:
        """The key of the file's security descriptor in `$Secure`. 0 for a file with no descriptor
        there (for example one with its own `$SECURITY_DESCRIPTOR`), or in the older layout."""
    @property
    def quota(self) -> int:
        """The bytes charged to the owner's quota. 0 if quotas have never been enabled on the
        volume, or in the older layout."""
    @property
    def usn(self) -> int:
        """The update sequence number of the file's last change journal (`$Extend\\$UsnJrnl`)
        record, which may no longer be in the journal. 0 if the file has had no record since the
        journal was created, or in the older layout."""
    @property
    def created(self) -> datetime.datetime: ...
    @property
    def modified(self) -> datetime.datetime: ...
    @property
    def mft_modified(self) -> datetime.datetime: ...
    @property
    def accessed(self) -> datetime.datetime: ...
    @property
    def file_flags(self) -> str:
        """The file's `FILE_ATTRIBUTE_*` flags."""
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
    @property
    def attribute_type(self) -> int: ...
    @property
    def lowest_vcn(self) -> int: ...
    @property
    def name(self) -> str: ...
    @property
    def segment_entry_id(self) -> int:
        """The entry that holds the attribute: this entry, or one of its extension records."""
    @property
    def segment_entry_sequence(self) -> int: ...

@final
class PyMftAttributeX30:
    """$FILE_NAME"""

    @property
    def parent_entry_id(self) -> int: ...
    @property
    def parent_entry_sequence(self) -> int: ...
    @property
    def logical_size(self) -> int: ...
    @property
    def physical_size(self) -> int: ...
    @property
    def reparse_value(self) -> int: ...
    @property
    def namespace(self) -> str: ...
    @property
    def name(self) -> str: ...
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

    @property
    def object_id(self) -> str:
        """Unique Id assigned to file."""
    @property
    def birth_volume_id(self) -> str:
        """Volume where file was created."""
    @property
    def birth_object_id(self) -> str:
        """Original Object Id of file."""
    @property
    def domain_id(self) -> str:
        """Domain in which object was created."""

@final
class PyMftAttributeX50:
    """$SECURITY_DESCRIPTOR: a resident security descriptor. Since NTFS 3.0 most files only
    have a security id in `$STANDARD_INFORMATION`, and the descriptor is in `$Secure`."""

    @property
    def revision(self) -> int: ...
    @property
    def control(self) -> str: ...
    @property
    def control_value(self) -> int:
        """`control` as an integer."""
    @property
    def owner(self) -> str | None:
        """The owner, as a SID string (`S-1-5-18`), or `None` when the descriptor has none."""
    @property
    def group(self) -> str | None: ...
    @property
    def sacl(self) -> PyMftAcl | None:
        """Only set when `control` has `SE_SACL_PRESENT`."""
    @property
    def dacl(self) -> PyMftAcl | None:
        """Only set when `control` has `SE_DACL_PRESENT`. A present flag without a DACL is a
        NULL DACL, which grants everyone full access."""

@final
class PyMftAcl:
    """An access control list."""

    @property
    def revision(self) -> int: ...
    def entries(self) -> list[PyMftAce]: ...

@final
class PyMftAce:
    """An access control entry."""

    @property
    def ace_type(self) -> int: ...
    @property
    def ace_type_name(self) -> str | None:
        """The name of a documented type (for example `ACCESS_ALLOWED`), without the
        `_ACE_TYPE` suffix."""
    @property
    def flags(self) -> str: ...
    @property
    def flags_value(self) -> int:
        """`flags` as an integer."""
    @property
    def access_mask(self) -> int | None:
        """`None` for types whose layout isn't decoded."""
    @property
    def object_type(self) -> str | None:
        """The object type of an object ACE, when its flags say it has one."""
    @property
    def inherited_object_type(self) -> str | None:
        """The inherited object type of an object ACE, when its flags say it has one."""
    @property
    def sid(self) -> str | None: ...
    @property
    def data(self) -> bytes:
        """The bytes after the decoded fields (for example a callback ACE's application
        data), or the whole body for types whose layout isn't decoded."""

@final
class PyMftAttributeX60:
    """$VOLUME_NAME: the volume label."""

    @property
    def name(self) -> str: ...

@final
class PyMftAttributeX70:
    """$VOLUME_INFORMATION: the NTFS version and volume state flags."""

    @property
    def major_version(self) -> int: ...
    @property
    def minor_version(self) -> int: ...
    @property
    def flags(self) -> str: ...
    @property
    def flags_value(self) -> int:
        """`flags` as an integer."""

@final
class PyMftAttributeX80:
    """$DATA (resident)"""

    @property
    def data(self) -> bytes: ...

@final
class PyMftAttributeX90:
    """$INDEX_ROOT"""

    @property
    def attribute_type(self) -> int: ...
    @property
    def collation_rule(self) -> int:
        """Collation rule used to sort the index entries."""
    @property
    def index_entry_size(self) -> int: ...
    @property
    def index_entry_number_of_cluster_blocks(self) -> int: ...
    @property
    def relative_offset_to_index_node(self) -> int: ...
    @property
    def index_node_length(self) -> int: ...
    @property
    def index_node_allocation_length(self) -> int: ...
    @property
    def is_large_index(self) -> bool:
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

    @property
    def mft_reference_entry_id(self) -> int: ...
    @property
    def mft_reference_entry_sequence(self) -> int: ...
    @property
    def index_record_length(self) -> int: ...
    @property
    def attr_fname_length(self) -> int: ...
    @property
    def flags(self) -> str: ...
    @property
    def fname_info(self) -> PyMftAttributeX30:
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

    @property
    def flags(self) -> str: ...
    @property
    def flags_value(self) -> int:
        """`flags` as an integer."""
    @property
    def value_type(self) -> str: ...
    @property
    def hash(self) -> int | None: ...
    @property
    def security_id(self) -> int | None: ...
    @property
    def sds_offset(self) -> int | None: ...
    @property
    def sds_length(self) -> int | None: ...
    @property
    def object_id(self) -> str | None: ...
    @property
    def mft_reference_entry_id(self) -> int | None: ...
    @property
    def mft_reference_entry_sequence(self) -> int | None: ...
    @property
    def birth_volume_id(self) -> str | None: ...
    @property
    def birth_object_id(self) -> str | None: ...
    @property
    def domain_id(self) -> str | None: ...
    @property
    def sid(self) -> str | None: ...
    @property
    def owner_id(self) -> int | None: ...
    @property
    def version(self) -> int | None: ...
    @property
    def quota_flags(self) -> int | None: ...
    @property
    def bytes_used(self) -> int | None: ...
    @property
    def warning_limit(self) -> int | None:
        """-1 means no limit."""
    @property
    def hard_limit(self) -> int | None:
        """-1 means no limit."""
    @property
    def tag(self) -> int | None: ...
    @property
    def tag_name(self) -> str | None:
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

    @property
    def tag(self) -> int:
        """The reparse tag, which says which file system filter owns the data."""
    @property
    def tag_name(self) -> str | None:
        """The name of a documented tag, such as `SYMLINK`, without the `IO_REPARSE_TAG_`
        prefix."""
    @property
    def guid(self) -> str | None:
        """The owning filter's GUID, for third-party tags."""
    @property
    def reparse_type(self) -> str: ...
    @property
    def substitute_name(self) -> str | None: ...
    @property
    def print_name(self) -> str | None: ...
    @property
    def is_relative(self) -> bool | None: ...
    @property
    def package_id(self) -> str | None: ...
    @property
    def app_user_model_id(self) -> str | None: ...
    @property
    def target_path(self) -> str | None: ...
    @property
    def target(self) -> str | None: ...
    @property
    def data(self) -> bytes | None:
        """The tag-specific data of an `Other` reparse point: the bytes after the reparse
        header (and the GUID of a third-party tag)."""

@final
class PyMftAttributeXD0:
    """$EA_INFORMATION: the sizes of the file's extended attributes in `$EA`."""

    @property
    def packed_ea_size(self) -> int:
        """The size of the extended attributes in packed form: each one's flags, lengths, name
        and value, without the next entry offsets and padding that `$EA` stores."""
    @property
    def need_ea_count(self) -> int:
        """How many extended attributes have the `NEED_EA` flag."""
    @property
    def unpacked_ea_size(self) -> int:
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

    @property
    def flags(self) -> str: ...
    @property
    def flags_value(self) -> int:
        """`flags` as an integer."""
    @property
    def name(self) -> str: ...
    @property
    def value_type(self) -> str: ...
    @property
    def value(self) -> int | None: ...
    @property
    def major(self) -> int | None: ...
    @property
    def minor(self) -> int | None: ...
    @property
    def catalog(self) -> str | None: ...
    @property
    def data(self) -> bytes | None:
        """The value of an `Other` entry."""

@final
class PyMftAttributeNonResident:
    """The content of a non-resident attribute: its sizes, and the data runs that locate it
    on the volume. The sizes are only valid in the attribute record whose `vcn_first` is 0."""

    @property
    def vcn_first(self) -> int:
        """The first virtual cluster number (VCN) this attribute record covers."""
    @property
    def vcn_last(self) -> int:
        """The last VCN this attribute record covers."""
    @property
    def allocated_length(self) -> int:
        """Allocated size in bytes, a multiple of the cluster size."""
    @property
    def file_size(self) -> int:
        """Real size in bytes."""
    @property
    def valid_data_length(self) -> int:
        """Initialized size in bytes. Reads past it return zeroes."""
    @property
    def unit_compression_size(self) -> int:
        """Compression unit size, as a power of two in clusters (0 if not compressed)."""
    @property
    def total_allocated(self) -> int | None:
        """Bytes actually allocated on disk. Only read for compressed attributes, `None`
        otherwise."""
    def data_runs(self) -> list[PyMftDataRun]: ...

@final
class PyMftDataRun:
    """A contiguous run of clusters. `lcn_offset` is the absolute cluster number where the run
    starts, and is 0 for sparse runs, which have no clusters on disk."""

    @property
    def lcn_offset(self) -> int: ...
    @property
    def lcn_length(self) -> int: ...
    @property
    def is_sparse(self) -> bool: ...

@final
class PyMftAttributeOther:
    """An attribute whose content is not parsed by the Rust library."""

    @property
    def data(self) -> bytes: ...
