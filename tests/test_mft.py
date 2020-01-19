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


def test_scoping(sample_mft):
    def resident_attribute():
        parser = PyMftParser(str(sample_mft))
        for entry in parser.entries():
            for attribute in entry.attributes():
                assert not isinstance(attribute, RuntimeError), (attribute, entry.entry_id, list(entry.attributes()))
                if attribute.type_code == 0x80 and attribute.attribute_content is not None:
                    if len(attribute.attribute_content.data) > 0:
                        return entry, attribute

    e, a = resident_attribute()
    assert a.attribute_content is not None
