import datetime

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
