import datetime
import itertools

import pytest

from pathlib import Path

from mft import PyMftParser, PyMftEntry


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

    for entry in parser.entries():
        try:
            for attribute in entry.attributes():
                print(entry.entry_id)
        except RuntimeError as e:
            assert False, (e, entry.entry_id)




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


def attribute_content(entry: PyMftEntry, type_code: int):
    return next(a for a in entry.attributes() if a.type_code == type_code).attribute_content


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
    entry = entry_by_id(sample_mft, 3)

    assert attribute_content(entry, 0x60).name == ""
    info = attribute_content(entry, 0x70)
    assert (info.major_version, info.minor_version) == (3, 1)
    assert info.flags == "(empty)"
    assert info.flags_value == 0


def reparse_point(entry: PyMftEntry):
    return attribute_content(entry, 0xC0)


def test_reparse_points():
    # Four records from the Narcos Windows 10 $MFT (Digital Corpora, CC0): entries 27557,
    # 28654, 84786 and 22487.
    path = Path(__file__).parent.parent / "samples" / "reparse_points"
    junction, symlink, app_exec_link, wci = (
        reparse_point(entry) for entry in PyMftParser(str(path)).entries()
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


def test_entry_header_extras(sample_mft: Path):
    parser = PyMftParser(str(sample_mft))

    mft = parser.get_entry(0)
    assert (mft.flags_value, mft.is_allocated, mft.is_dir) == (1, True, False)
    assert mft.lsn == 77648146
    assert mft.valid_fixup is True

    root = parser.get_entry(5)
    assert (root.flags_value, root.is_dir) == (3, True)


def test_flag_values(sample_mft: Path):
    entry = entry_by_id(sample_mft, 0)
    attribute = next(entry.attributes())
    assert attribute.data_flags_value == 0
    # FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM
    assert attribute.attribute_content.file_flags_value == 0x6
    assert attribute_content(entry, 0x30).flags_value == 0x6


def test_non_resident_sizes(sample_mft: Path):
    data = attribute_content(entry_by_id(sample_mft, 0), 0x80)

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
