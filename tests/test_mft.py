import pytest

from pathlib import Path

from mft import PyMftParser

@pytest.fixture
def sample_mft() -> str:
    p = Path(__file__).parent.parent / "samples" / "MFT"
    assert p.exists()

    return p


def test_it_works(sample_mft):
    with open(sample_mft, "rb") as m:
        parser = PyMftParser(m)

        for record in parser.entries():
            print(record)