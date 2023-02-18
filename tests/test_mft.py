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


