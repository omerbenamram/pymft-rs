import datetime
import io
import itertools

import pytest

from pathlib import Path

from mft import PyMftParser, PyMftEntry

ENTRY_SIZE = 1024


@pytest.fixture
def sample_mft() -> Path:
    p = Path(__file__).parent.parent / "samples" / "MFT"
    assert p.exists()

    return p


def test_it_works(sample_mft: Path):
    with open(sample_mft, "rb") as m:
        parser = PyMftParser(m)

        sample_record: PyMftEntry = next(parser.entries())

        assert sample_record.entry_id == 0
        assert sample_record.full_path == "$MFT"


def test_iter_attributes(sample_mft: Path):
    with open(sample_mft, "rb") as m:
        parser = PyMftParser(m)

        sample_record: PyMftEntry = next(parser.entries())

        l = list(sample_record.attributes())
        assert len(l) == 4


def test_datetimes_are_converted_properly(sample_mft: Path):
    with open(sample_mft, "rb") as m:
        parser = PyMftParser(m)

        sample_record: PyMftEntry = next(parser.entries())

        attribute = next(sample_record.attributes())

        content = attribute.attribute_content

        assert content.created.tzinfo == datetime.timezone.utc


def test_doesnt_yield_zeroed_entries(sample_mft: Path):
    parser = PyMftParser(str(sample_mft))

    # Reading a zeroed record's attributes would raise.
    for entry in parser.entries():
        for _attribute in entry.attributes():
            pass




def test_datetime_values(sample_mft: Path):
    parser = PyMftParser(str(sample_mft))
    sample_record: PyMftEntry = next(parser.entries())
    content = next(sample_record.attributes()).attribute_content

    expected = datetime.datetime(2007, 6, 30, 12, 50, 52, 252395, tzinfo=datetime.timezone.utc)
    assert content.created == expected
    assert content.modified == expected
    assert content.mft_modified == expected
    assert content.accessed == expected


def test_entry_header_fields(sample_mft: Path):
    parser = PyMftParser(str(sample_mft))

    entry: PyMftEntry = next(parser.entries())

    assert entry.hard_link_count == 1
    assert entry.flags == "ALLOCATED"

    attribute = next(entry.attributes())
    assert attribute.attribute_content.file_flags == "FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM"
    assert attribute.data_flags == "(empty)"


def entry_by_id(sample_mft: Path, entry_id: int) -> PyMftEntry:
    for entry in PyMftParser(str(sample_mft)).entries():
        if entry.entry_id == entry_id:
            return entry
    raise AssertionError(f"entry {entry_id} not found")


def attribute(entry: PyMftEntry, type_code: int):
    return next(a for a in entry.attributes() if a.type_code == type_code)


def attribute_content(entry: PyMftEntry, type_code: int):
    return attribute(entry, type_code).attribute_content


def test_full_path_uses_backslashes(sample_mft: Path):
    assert entry_by_id(sample_mft, 29).full_path == "WINDOWS\\system32"


def test_non_resident_data_runs(sample_mft: Path):
    content = attribute_content(entry_by_id(sample_mft, 0), 0x80)

    runs = [(r.lcn_offset, r.lcn_length, r.is_sparse) for r in content.data_runs()]
    assert runs == [
        (205374, 4536, False),
        (261023, 1288, False),
        (230736, 336, False),
        (399772, 384, False),
    ]


def test_sparse_data_run(sample_mft: Path):
    # $BadClus:$Bad covers the whole volume with one sparse run.
    entry = entry_by_id(sample_mft, 8)
    bad = next(a for a in entry.attributes() if a.type_code == 0x80 and a.name == "$Bad")

    runs = [(r.lcn_offset, r.lcn_length, r.is_sparse) for r in bad.attribute_content.data_runs()]
    assert runs == [(0, 616124, True)]


def test_index_root_entries(sample_mft: Path):
    content = attribute_content(entry_by_id(sample_mft, 29), 0x90)

    assert content.is_large_index
    assert content.index_node_length == 392

    entries = content.index_entries()
    assert len(entries) == 3
    first = entries[0]
    assert first.mft_reference_entry_id == 595
    assert first.mft_reference_entry_sequence == 1
    assert first.flags == "INDEX_ENTRY_NODE"
    assert first.fname_info.name == "eventtriggers.exe"
    assert first.fname_info.parent_entry_id == 29


def test_volume_attributes(sample_mft: Path):
    entry = PyMftParser(str(sample_mft)).get_entry(3)

    assert attribute_content(entry, 0x60).name == ""
    info = attribute_content(entry, 0x70)
    assert (info.major_version, info.minor_version) == (3, 1)
    assert info.flags == "(empty)"
    assert info.flags_value == 0
    # 8 reserved bytes, then NTFS 3.1 and no volume flags.
    assert attribute(entry, 0x70).raw_value == b"\x00" * 8 + b"\x03\x01\x00\x00"
    # Non-resident $DATA has no resident value.
    assert attribute(PyMftParser(str(sample_mft)).get_entry(0), 0x80).raw_value is None


def test_reparse_points():
    # Four records from the Narcos Windows 10 $MFT (Digital Corpora, CC0): entries 27557,
    # 28654, 84786 and 22487.
    path = Path(__file__).parent.parent / "samples" / "reparse_points"
    junction, symlink, app_exec_link, wci = (
        attribute_content(entry, 0xC0) for entry in PyMftParser(str(path)).entries()
    )

    # `Users\Default User`
    assert (junction.tag, junction.tag_name, junction.reparse_type) == (
        0xA0000003,
        "MOUNT_POINT",
        "MountPoint",
    )
    assert junction.substitute_name == "\\??\\C:\\Users\\Default"
    assert junction.print_name == "C:\\Users\\Default"
    assert junction.is_relative is None
    assert junction.data is None

    # `Users\All Users`
    assert symlink.reparse_type == "SymbolicLink"
    assert symlink.print_name == "C:\\ProgramData"
    assert symlink.is_relative is False

    # `WindowsApps\MicrosoftEdge.exe`
    assert app_exec_link.reparse_type == "AppExecLink"
    assert app_exec_link.package_id == "Microsoft.MicrosoftEdge_8wekyb3d8bbwe"
    assert app_exec_link.app_user_model_id == "Microsoft.MicrosoftEdge_8wekyb3d8bbwe!MicrosoftEdge"
    assert app_exec_link.target_path == "C:\\Windows\\System32\\SystemUWPLauncher.exe"
    assert app_exec_link.substitute_name is None

    # A Windows Container Isolation placeholder: a documented tag with no decoder.
    assert (wci.tag_name, wci.reparse_type, wci.guid) == ("WCI", "Other", None)
    assert wci.data == bytes.fromhex("01000000000000007a04b779e4f5ef428db10e504254f2c90000")


def test_security_descriptor(sample_mft: Path):
    # $Volume keeps its descriptor in the record.
    descriptor = attribute_content(PyMftParser(str(sample_mft)).get_entry(3), 0x50)
    assert descriptor.revision == 1
    assert descriptor.control == "SE_DACL_PRESENT | SE_SELF_RELATIVE"
    assert descriptor.control_value == 0x8004
    assert (descriptor.owner, descriptor.group) == ("S-1-5-32-544", "S-1-5-32-544")
    assert descriptor.sacl is None
    assert descriptor.dacl.revision == 2
    system, admins = descriptor.dacl.entries()
    assert (system.ace_type, system.ace_type_name, system.sid) == (0, "ACCESS_ALLOWED", "S-1-5-18")
    assert (system.flags, system.flags_value) == ("(empty)", 0)
    assert system.access_mask == 0x12019F
    assert system.object_type is None
    assert system.data == b""
    assert admins.sid == "S-1-5-32-544"


def test_view_index_entries(sample_mft: Path):
    parser = PyMftParser(str(sample_mft))

    # $Secure: one entry in the root of each of its indexes.
    sdh, sii = (
        a.attribute_content.view_entries()
        for a in parser.get_entry(9).attributes()
        if a.type_code == 0x90
    )
    assert [(e.value_type, e.hash, e.security_id, e.sds_offset, e.sds_length) for e in sdh] == [
        ("SecurityDescriptor", 0x8048FB19, 301, 9648, 236)
    ]
    assert (sdh[0].flags, sdh[0].flags_value) == ("INDEX_ENTRY_NODE", 1)
    assert [(e.hash, e.security_id) for e in sii] == [(0x63D46E04, 305)]
    assert sdh[0].owner_id is None and sdh[0].change_time is None and sdh[0].key is None

    # $Extend\$Quota: the owner id of the Administrators SID, and the quota entries.
    owners, quotas = (
        a.attribute_content.view_entries()
        for a in parser.get_entry(24).attributes()
        if a.type_code == 0x90
    )
    assert [(e.value_type, e.sid, e.owner_id) for e in owners] == [
        ("QuotaOwner", "S-1-5-32-544", 256)
    ]
    default, admins = quotas
    assert (default.value_type, default.owner_id, default.sid) == ("Quota", 1, None)
    assert (admins.owner_id, admins.sid) == (256, "S-1-5-32-544")
    assert (admins.version, admins.quota_flags, admins.bytes_used) == (2, 1, 0)
    assert (admins.warning_limit, admins.hard_limit) == (-1, -1)
    assert admins.change_time == datetime.datetime(
        2007, 6, 30, 12, 50, 53, 233806, tzinfo=datetime.timezone.utc
    )
    assert admins.exceeded_time is None

    # $Extend\$ObjId
    (object_id,) = attribute_content(parser.get_entry(25), 0x90).view_entries()
    assert object_id.value_type == "ObjectId"
    assert object_id.object_id == "9FE44B58-2709-11DC-A06B-DB3099BEAE3C"
    assert (object_id.mft_reference_entry_id, object_id.mft_reference_entry_sequence) == (5321, 2)
    assert object_id.birth_volume_id == "00000000-0000-0000-0000-000000000000"

    # A directory's $I30 index has no view entries.
    system32 = attribute_content(parser.get_entry(29), 0x90)
    assert system32.view_entries() == []
    assert system32.index_entries()


def test_extended_attributes():
    # Record 87354 (`Windows/System32/vm3dum64.dll`) of the Narcos Windows 10 $MFT (Digital
    # Corpora, CC0).
    path = Path(__file__).parent.parent / "samples" / "entry_87354_extended_attributes"
    entry = next(PyMftParser(str(path)).entries())

    info = attribute_content(entry, 0xD0)
    assert (info.packed_ea_size, info.need_ea_count, info.unpacked_ea_size) == (168, 0, 180)

    catalog_hint, cache = attribute_content(entry, 0xE0).entries()
    assert (catalog_hint.name, catalog_hint.value_type) == ("$CI.CATALOGHINT", "CatalogHint")
    assert catalog_hint.catalog == "oem10.cat"
    assert (catalog_hint.flags, catalog_hint.flags_value) == ("(empty)", 0)
    assert catalog_hint.data is None
    assert (cache.name, cache.value_type, cache.catalog) == ("$KERNEL.PURGE.ESBCACHE", "Other", None)
    assert len(cache.data) == 0x6C
    assert cache.data[:4] == b"\x6c\x00\x00\x00"


def test_entry_header_extras(sample_mft: Path):
    parser = PyMftParser(str(sample_mft))

    mft = parser.get_entry(0)
    assert (mft.flags_value, mft.is_allocated, mft.is_dir) == (1, True, False)
    assert mft.lsn == 77648146
    assert mft.valid_fixup is True

    root = parser.get_entry(5)
    assert (root.flags_value, root.is_dir) == (3, True)


def test_flag_values(sample_mft: Path):
    entry = PyMftParser(str(sample_mft)).get_entry(0)
    attribute = next(entry.attributes())
    assert attribute.data_flags_value == 0
    # FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM
    assert attribute.attribute_content.file_flags_value == 0x6
    assert attribute_content(entry, 0x30).flags_value == 0x6


def test_non_resident_sizes(sample_mft: Path):
    data = attribute_content(PyMftParser(str(sample_mft)).get_entry(0), 0x80)

    assert (data.vcn_first, data.vcn_last) == (0, 6543)
    assert data.allocated_length == data.file_size == data.valid_data_length == 13402112
    assert data.unit_compression_size == 0
    assert data.total_allocated is None


def test_named_resident_attribute_data(sample_mft: Path):
    entry = entry_by_id(sample_mft, 9)
    sdh = next(a for a in entry.attributes() if a.type_code == 0xB0 and a.name == "$SDH")

    assert sdh.attribute_content.data == b"\x03" + b"\x00" * 7


def test_attribute_list_entries(sample_mft: Path):
    content = attribute_content(entry_by_id(sample_mft, 3498), 0x20)

    entries = [(e.attribute_type, e.lowest_vcn, e.name) for e in content.entries()]
    assert entries == [(0x10, 0, ""), (0x30, 0, ""), (0x80, 0, "")]
    # $DATA lives in extension record 4711.
    segments = [(e.segment_entry_id, e.segment_entry_sequence) for e in content.entries()]
    assert segments == [(3498, 1), (3498, 1), (4711, 2)]
    # The entries can be iterated again.
    assert len(list(content.entries())) == 3


def test_parser_can_be_iterated_more_than_once(sample_mft: Path):
    parser = PyMftParser(str(sample_mft))

    first = [entry.entry_id for entry in parser.entries()]
    second = [entry.entry_id for entry in parser.entries()]

    assert first == second
    assert len(next(iter(parser.entries_json()))) > 0
    assert parser.number_of_entries() == 13088


def test_get_entry(sample_mft: Path):
    parser = PyMftParser(str(sample_mft))

    root = parser.get_entry(5)
    assert root.entry_id == 5
    assert root.full_path == "."
    assert root.flags == "ALLOCATED | INDEX_PRESENT"

    iterated = entry_by_id(sample_mft, 46)
    looked_up = parser.get_entry(46)
    assert looked_up.full_path == iterated.full_path
    assert looked_up.file_size == iterated.file_size
    assert len(list(looked_up.attributes())) == len(list(iterated.attributes()))


def test_get_entry_while_iterating(sample_mft: Path):
    parser = PyMftParser(str(sample_mft))
    entries = parser.entries()

    assert next(entries).entry_id == 0
    assert parser.get_entry(5).full_path == "."
    # The iterator carries on from where it was.
    assert next(entries).entry_id == 1


def test_get_entry_of_a_zeroed_record(sample_mft: Path):
    parser = PyMftParser(str(sample_mft))

    # Entry 16's record is all zeroes, so `entries()` skips it.
    assert 16 not in [entry.entry_id for entry in itertools.islice(parser.entries(), 20)]
    assert parser.get_entry(16) is None


def test_get_entry_past_the_end(sample_mft: Path):
    parser = PyMftParser(str(sample_mft))

    with pytest.raises(IndexError):
        parser.get_entry(parser.number_of_entries())
    with pytest.raises(IndexError):
        parser.get_entry(-1)


@pytest.fixture
def mft_with_a_bad_entry(sample_mft: Path, tmp_path: Path) -> Path:
    """`samples/MFT` with entry 3's signature broken."""
    data = bytearray(sample_mft.read_bytes())
    data[3 * ENTRY_SIZE : 3 * ENTRY_SIZE + 4] = b"XXXX"
    path = tmp_path / "MFT"
    path.write_bytes(data)
    return path


@pytest.fixture
def mft_with_a_bad_attribute(sample_mft: Path, tmp_path: Path) -> Path:
    """`samples/MFT` with an unknown type code in entry 0's first attribute."""
    data = bytearray(sample_mft.read_bytes())
    first_attribute = int.from_bytes(data[0x14:0x16], "little")
    data[first_attribute : first_attribute + 4] = (0x11).to_bytes(4, "little")
    path = tmp_path / "MFT"
    path.write_bytes(data)
    return path


def test_a_bad_entry_raises_and_iteration_goes_on(mft_with_a_bad_entry: Path):
    entries = PyMftParser(str(mft_with_a_bad_entry)).entries()

    assert [next(entries).entry_id for _ in range(3)] == [0, 1, 2]
    with pytest.raises(RuntimeError, match="signature"):
        next(entries)
    assert next(entries).entry_id == 4


@pytest.mark.parametrize("method", ["entries", "entries_json", "entries_csv"])
def test_skip_errors_skips_a_bad_entry(mft_with_a_bad_entry: Path, method: str):
    parser = PyMftParser(str(mft_with_a_bad_entry), skip_errors=True)

    with pytest.warns(RuntimeWarning, match="^Skipping entry 3: RuntimeError: ") as record:
        records = list(getattr(parser, method)())

    assert len(record) == 1
    # One fewer than the 13,068 entries of `samples/MFT` that aren't zeroed.
    assert len(records) == 13_067


@pytest.mark.filterwarnings("error::RuntimeWarning")
def test_skip_errors_warnings_can_be_errors(mft_with_a_bad_entry: Path):
    parser = PyMftParser(str(mft_with_a_bad_entry), skip_errors=True)

    with pytest.raises(RuntimeWarning, match="entry 3") as raised:
        list(parser.entries())

    assert isinstance(raised.value.__cause__, RuntimeError)


def test_a_bad_attribute_raises_and_iteration_goes_on(mft_with_a_bad_attribute: Path):
    attributes = PyMftParser(str(mft_with_a_bad_attribute)).get_entry(0).attributes()

    with pytest.raises(RuntimeError, match="Unknown `AttributeType`"):
        next(attributes)
    assert [a.type_name for a in attributes] == ["FileName", "DATA", "BITMAP"]


def test_skip_errors_skips_a_bad_attribute(mft_with_a_bad_attribute: Path):
    entry = next(PyMftParser(str(mft_with_a_bad_attribute), skip_errors=True).entries())

    with pytest.warns(RuntimeWarning, match="^Skipping attribute 0 of entry 0: "):
        assert [a.type_name for a in entry.attributes()] == ["FileName", "DATA", "BITMAP"]


def test_skip_errors_still_raises_read_errors(sample_mft: Path):
    class FailingFile(io.BytesIO):
        """Fails to read past the first 32 entries."""

        def read(self, size: int | None = -1) -> bytes:
            if self.tell() >= 32 * ENTRY_SIZE:
                raise OSError("bad sector")
            return super().read(size)

    parser = PyMftParser(FailingFile(sample_mft.read_bytes()), skip_errors=True)

    with pytest.raises(OSError, match="bad sector"):
        list(parser.entries())
