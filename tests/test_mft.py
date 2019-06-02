import datetime

import pytest

from pathlib import Path

from mft import PyMftParser, PyMftEntry


@pytest.fixture
def sample_mft() -> str:
    p = Path(__file__).parent.parent / "samples" / "MFT"
    assert p.exists()

    return p


def test_it_works(sample_mft):
    with open(sample_mft, "rb") as m:
        parser = PyMftParser(m)

        sample_record: PyMftEntry = next(parser.entries())

        assert sample_record.full_path == "$MFT"


def test_iter_attributes(sample_mft):
    with open(sample_mft, "rb") as m:
        parser = PyMftParser(m)

        sample_record: PyMftEntry = next(parser.entries())

        l = list(sample_record.attributes())
        assert len(l) == 4


def test_datetimes_are_converted_properly(sample_mft):
    with open(sample_mft, "rb") as m:
        parser = PyMftParser(m)

        sample_record: PyMftEntry = next(parser.entries())

        attribute = next(sample_record.attributes())

        content = attribute.attribute_content

        assert content.created.tzinfo == datetime.timezone.utc